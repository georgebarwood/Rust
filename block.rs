use crate::matcher::Match;
use crate::bit::BitStream;
use crate::bit::BitCoder;
use crate::bit::LenCoder;

pub struct Block
{
  input_start: usize, pub input_end: usize,
  match_start: usize, pub match_end: usize,
  lit: BitCoder, dist: BitCoder, len: LenCoder,
  len_symbols: usize,
  bits_computed: bool,
}
 
impl Block
{
  pub fn new( input_start: usize, input_count: usize, match_start: usize  ) -> Block
  {
    Block
    { 
      input_start, 
      input_end: input_start + input_count, 
      match_start,
      match_end: 0,
      lit:  BitCoder::new( 15, 288 ), 
      dist: BitCoder::new( 15, 32 ), 
      len:  LenCoder::new( 7, 19 ),
      len_symbols: 0,
      bits_computed: false,
    }
  }

  pub fn init( &mut self, input: &[u8], mlist: &[Match] )
  {
    // Counts how many times each symbol is used, also determines exact end of block.

    let mut position : usize = self.input_start;

    let mut mi = self.match_start; 
    loop // Through the applicable matches.
    {
      if mi == mlist.len() { break; }

      let mat = &mlist[ mi ];

      if mat.position >= self.input_end { break; }

      while position < mat.position
      {
        self.lit.used[ input[ position ] as usize ] += 1;
        position += 1;
      }

      // Compute match and distance codes.
      position += mat.length;
      let mut mc = 0; while mat.length >= MATCH_OFF[ mc ] { mc += 1; } mc -= 1;
      let mut dc = 29; while mat.distance < DIST_OFF[ dc ] { dc -= 1; }

      self.lit.used[ 257 + mc ] += 1;
      self.dist.used[ dc ] += 1;

      mi += 1;  
    }
    self.match_end = mi;

    while position < self.input_end
    {
      self.lit.used[ input[ position ] as usize ] += 1;
      position += 1;
    }

    self.input_end = position;
    self.lit.used[ 256 ] += 1; // End of block code.
  }

  pub fn bit_size( &mut self, output: &mut BitStream ) -> usize
  { 
    self.compute_bits( output );
    17 + 3 * self.len_symbols + self.len.bc.total() + self.lit.total() + self.dist.total()
  }

  pub fn write( &mut self, input: &[u8], mlist: &[Match], output: &mut BitStream, last: bool )
  {
    self.bit_size( output );
    self.lit.compute_codes();
    self.dist.compute_codes();
    self.len.bc.compute_codes();

    output.write( 1, if last {1} else {0} );

    output.write( 2, 2 );
    output.write( 5, ( self.lit.symbols - 257 ) as u64 ); 
    output.write( 5, ( self.dist.symbols - 1 ) as u64 ); 
    output.write( 4, ( self.len_symbols - 4 ) as u64 );

    for alp in &CLEN_ALPHABET[..self.len_symbols]
    {
      output.write( 3, self.len.bc.bits[ *alp as usize ] as u64 );
    }

    self.do_length_pass( 2, output );
    self.put_codes( input, mlist, output );
    output.write( self.lit.bits[ 256 ], self.lit.code[ 256 ] as u64 ); // End of block code
  }

  fn put_codes( &mut self, input: &[u8], mlist: &[Match], output: &mut BitStream )
  {
    let mut position = self.input_start;

    for mat in &mlist[self.match_start .. self.match_end]
    {
      while position < mat.position
      {
        let ib = input[ position ] as usize;
        output.write( self.lit.bits[ ib ], self.lit.code[ ib ] as u64 );
        position += 1;
      }

      // Compute match and distance codes.
      position += mat.length;
      let mut mc = 0; while mat.length >= MATCH_OFF[ mc ] { mc += 1; } mc -= 1;
      let mut dc = 29; while mat.distance < DIST_OFF[ dc ] { dc -= 1; }

      // Output match info.
      output.write( self.lit.bits[ 257 + mc ], self.lit.code[ 257 + mc ] as u64 );
      output.write( MATCH_EXTRA[ mc ], ( mat.length - MATCH_OFF[ mc ] ) as u64 );
      output.write( self.dist.bits[ dc ], self.dist.code[ dc ] as u64 );
      output.write( DIST_EXTRA[ dc ], ( mat.distance - DIST_OFF[ dc ] ) as u64 );  
    }  

    while position < self.input_end
    {
      let ib = input[ position ] as usize;
      output.write( self.lit.bits[ ib ], self.lit.code[ ib ] as u64 );
      position += 1;
    }
  }

  fn compute_bits( &mut self, output: &mut BitStream )
  {
    if self.bits_computed { return; }      

    self.lit.compute_bits();
    self.dist.compute_bits();

    if self.dist.symbols == 0 { self.dist.symbols = 1; }

    // Compute length encoding.
    self.do_length_pass( 1, output );
    self.len.bc.compute_bits();

    // The length codes are permuted before being stored ( so that # of trailing zeroes is likely to be more ).
    self.len_symbols = 19; 
    while self.len_symbols > 4 
      && self.len.bc.bits[ CLEN_ALPHABET[ self.len_symbols - 1 ] as usize ] == 0
    {
      self.len_symbols -= 1;
    }

    self.bits_computed = true;
  }

  fn do_length_pass( &mut self, pass: u8, output: &mut BitStream )
  {
    self.len.length_pass = pass; 
    self.len.encode_lengths( true, self.lit.symbols, &self.lit.bits, output );     
    self.len.encode_lengths( false, self.dist.symbols, &self.dist.bits, output );
  }

} // end impl Block

// RFC 1951 constants.

pub static CLEN_ALPHABET : [u8; 19] = [ 16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15 ];

pub static MATCH_OFF : [usize; 30] = [ 3,4,5,6, 7,8,9,10, 11,13,15,17, 19,23,27,31, 35,43,51,59, 
  67,83,99,115,  131,163,195,227, 258, 0xffff ];

pub static MATCH_EXTRA : [u8; 29] = [ 0,0,0,0, 0,0,0,0, 1,1,1,1, 2,2,2,2, 3,3,3,3, 4,4,4,4, 5,5,5,5, 0 ];

pub static DIST_OFF : [usize; 30] = [ 1,2,3,4, 5,7,9,13, 17,25,33,49, 65,97,129,193, 257,385,513,769, 
  1025,1537,2049,3073, 4097,6145,8193,12289, 16385,24577 ];

pub static DIST_EXTRA : [u8; 30] = [ 0,0,0,0, 1,1,2,2, 3,3,4,4, 5,5,6,6, 7,7,8,8, 9,9,10,10, 11,11,12,12, 13,13 ];