// RFC 1951 inflate ( de-compress ).

pub fn inflate( data: &[u8] ) -> Vec<u8>
{
  let mut inp = InpBitStream::new( &data );
  let mut out = Vec::new();
  let _chk = inp.get_bits( 16 ); // Checksum
  loop
  {
    let last = inp.get_bit();
    let btype = inp.get_bits( 2 );
    match btype
    {
      2 => { do_dyn( &mut inp, &mut out ); }
      1 => { do_fixed( &mut inp, &mut out ); }
      0 => { do_copy( &mut inp, &mut out ); }
      _ => { }
    }
    if last != 0 { break; }
  }  
  out
}

fn do_dyn( inp: &mut InpBitStream, out: &mut Vec<u8> )
{
  let n_lit_code = 257 + inp.get_bits( 5 );
  let n_dist_code = 1 + inp.get_bits( 5 );
  let n_len_code = 4 + inp.get_bits( 4 );

  let mut len = LenDecoder::new( inp, n_len_code );

  let mut lit = BitDecoder::new( n_lit_code );
  len.get_lengths( inp, &mut lit.nbits );
  lit.init(); 

  let mut dist = BitDecoder::new( n_dist_code );
  len.get_lengths( inp, &mut dist.nbits );
  dist.init();

  loop
  {
    let x = lit.decode( inp );
    match x
    {
      0..=255 => { out.push( x as u8 ); }
      256 =>  { break; } 
      _ =>
      {
        let mc = x - 257;
        let length = MATCH_OFF[ mc ] + inp.get_bits( MATCH_EXTRA[ mc ] as usize );
        let dc = dist.decode( inp );
        let distance = DIST_OFF[ dc ] + inp.get_bits( DIST_EXTRA[ dc ] as usize );
        copy( out, distance, length ); 
      }
    }
  }
} // end do_dyn

fn copy( out: &mut Vec<u8>, distance: usize, mut length: usize )
{
  let mut i = out.len() - distance;
  while length > 0
  {
    out.push( out[ i ] );
    i += 1;
    length -= 1;
  }
}

/// Decode length-limited Huffman codes.
struct BitDecoder
{
  ncode: usize,
  nbits: Vec<usize>,
  maxbits: usize,
  peekbits: usize,
  lookup: Vec<usize>
}

impl BitDecoder
{
  fn new( ncode: usize ) -> BitDecoder
  {
    BitDecoder 
    { 
      ncode,
      nbits: vec![0; ncode],
      maxbits: 0,
      peekbits: 0,
      lookup: Vec::new()
    }
  }

  /// The key routine, will be called many times.
  fn decode( &self, input: &mut InpBitStream ) -> usize
  {
    let mut sym = self.lookup[ input.peek( self.peekbits ) ];
    if sym >= self.ncode
    {
      sym = self.lookup[ sym - self.ncode + ( input.peek( self.maxbits ) >> self.peekbits ) ];
    }  
    input.advance( self.nbits[ sym ] );
    sym
  }

  fn init( &mut self )
  {
    let ncode = self.ncode;

    let mut max_bits : usize = 0; 
    for bits in &self.nbits 
    { 
      if *bits > max_bits { max_bits = *bits; } 
    }

    self.maxbits = max_bits;
    self.peekbits = if max_bits > 8 { 8 } else { max_bits };
    self.lookup.resize( 1 << self.peekbits, 0 );

    // Code below is from rfc1951 page 7

    let mut bl_count : Vec<usize> = vec![ 0; max_bits + 1 ]; // the number of codes of length N, N >= 1.

    for i in 0..ncode { bl_count[ self.nbits[i] ] += 1; }

    let mut next_code : Vec<usize> = vec![ 0; max_bits + 1 ];
    let mut code = 0; 
    bl_count[0] = 0;

    for i in 0..max_bits
    {
      code = ( code + bl_count[i] ) << 1;
      next_code[ i + 1 ] = code;
    }

    for i in 0..ncode
    {
      let len = self.nbits[ i ];
      if len != 0
      {
        self.setup_code( i, len, next_code[ len ] );
        next_code[ len ] += 1;
      }
    }
  }

  // Decoding is done using self.lookup ( see decode ). To keep the lookup table small,
  // codes longer than 8 bits are looked up in two peeks.

  fn setup_code( &mut self, sym: usize, len: usize, mut code: usize )
  {
    if len <= self.peekbits
    {
      let diff = self.peekbits - len;
      for i in code << diff .. (code << diff) + (1 << diff)
      {
        // bits are reversed to match InpBitStream::peek
        let r = reverse( i, self.peekbits );
        self.lookup[ r ] = sym;
      }
    } else {
      // Secondary lookup required.
      let peekbits2 = self.maxbits - self.peekbits;

      // Split code into peekbits portion ( key ) and remainder ( code).
      let diff1 = len - self.peekbits;
      let key = code >> diff1;
      code &= ( 1 << diff1 ) - 1;

      // Get the secondary lookup.
      let kr = reverse( key, self.peekbits );
      let mut base = self.lookup[ kr ];
      if base == 0 // Secondary lookup not yet allocated for this key.
      {
        base = self.lookup.len();
        self.lookup.resize( base + ( 1 << peekbits2 ), 0 );
        self.lookup[ kr ] = self.ncode + base;
      } else {
        base -= self.ncode;
      }

      // Set the secondary lookup values.
      let diff = self.maxbits - len;
      for i in code << diff .. (code << diff) + (1<<diff)
      { 
        let r = reverse( i, peekbits2 );
        self.lookup[ base + r ] = sym;
      }
    }    
  }
} // end impl BitDecoder

struct InpBitStream<'a>
{
  data: &'a [u8],
  pos: usize,
  buf: usize,
  got: usize, // Number of bits in buffer.
}

impl <'a> InpBitStream<'a>
{
  fn new( data: &'a [u8] ) -> InpBitStream
  {
    InpBitStream { data, pos: 0, buf: 1, got: 0 }
  } 

  fn peek( &mut self, n: usize ) -> usize
  {
    while self.got < n
    {
      self.buf |= ( self.data[ self.pos ] as usize ) << self.got;
      self.pos += 1;
      self.got += 8;
    }
    self.buf & ( ( 1 << n ) - 1 )
  }

  fn advance( &mut self, n:usize )
  { 
    self.buf >>= n;
    self.got -= n;
  }

  fn get_bit( &mut self ) -> usize
  {
    if self.got == 0 { self.peek( 1 ); }
    let result = self.buf & 1;
    self.advance( 1 );
    result
  }

  fn get_bits( &mut self, n: usize ) -> usize
  { 
    let mut result = 0; 
    for i in 0..n
    {
      result |= self.get_bit() << i; 
    }
    result
  }

  fn get_huff( &mut self, mut n: usize ) -> usize 
  { 
    let mut result = 0; 
    while n > 0
    { 
      result = ( result << 1 ) + self.get_bit(); 
      n -= 1;
    }
    result
  }

  fn clear_bits( &mut self )
  {
    self.got = 0;
  }
} //  end impl InpBitStream

/// Decode code lengths.
struct LenDecoder
{
  plenc: usize, // previous length code ( which can be repeated )
  rep: usize,   // repeat
  bd: BitDecoder,
}

/// Decodes an array of lengths. There are special codes for repeats, and repeats of zeros.
impl LenDecoder
{
  fn new( inp: &mut InpBitStream, n_len_code: usize ) -> LenDecoder
  {
    let mut result = LenDecoder { plenc: 0, rep:0, bd: BitDecoder::new( 19 ) };

    // Read the array of 3-bit code lengths from input.
    for i in 0..n_len_code 
    { 
      result.bd.nbits[ CLEN_ALPHABET[i] as usize ] = inp.get_bits(3); 
    }
    result.bd.init();
    result
  }

  // Per RFC1931 page 13, get array of code lengths.
  fn get_lengths( &mut self, inp: &mut InpBitStream, result: &mut Vec<usize> )
  {
    let n = result.len();
    let mut i = 0;
    while self.rep > 0 { result[i] = self.plenc; i += 1; self.rep -= 1; }
    while i < n
    { 
      let lenc = self.bd.decode( inp );
      if lenc < 16 
      {
        result[i] = lenc; 
        i += 1; 
        self.plenc = lenc; 
      } else {
        if lenc == 16 { self.rep = 3 + inp.get_bits(2); }
        else if lenc == 17 { self.rep = 3 + inp.get_bits(3); self.plenc=0; }
        else if lenc == 18 { self.rep = 11 + inp.get_bits(7); self.plenc=0; } 
        while i < n && self.rep > 0 { result[i] = self.plenc; i += 1; self.rep -= 1; }
      }
    }
  } // end get_lengths
} // end impl LenDecoder

/// Reverse a string of bits.
pub fn reverse( mut x:usize, mut bits: usize ) -> usize
{ 
  let mut result: usize = 0; 
  while bits > 0
  {
    result = ( result << 1 ) | ( x & 1 ); 
    x >>= 1; 
    bits -= 1;
  } 
  result
} 

fn do_copy( inp: &mut InpBitStream, out: &mut Vec<u8> )
{
  inp.clear_bits(); // Discard any bits in the input buffer
  let mut n = inp.get_bits( 16 );
  let _n1 = inp.get_bits( 16 );
  while n > 0 { out.push( inp.data[ inp.pos ] ); n -= 1; inp.pos += 1; }
}

fn do_fixed( inp: &mut InpBitStream, out: &mut Vec<u8> ) // RFC1951 page 12.
{
  loop
  {
    // 0 to 23 ( 7 bits ) => 256 - 279; 48 - 191 ( 8 bits ) => 0 - 143; 
    // 192 - 199 ( 8 bits ) => 280 - 287; 400..511 ( 9 bits ) => 144 - 255
    let mut x = inp.get_huff( 7 ); 
    if x <= 23 
    { 
      x += 256; 
    } else {
      x = ( x << 1 ) + inp.get_bit();
      if x <= 191 { x -= 48; }
      else if x <= 199 { x += 88; }
      else { x = ( x << 1 ) + inp.get_bit() - 256; }
    }

    match x
    {
      0..=255 => { out.push( x as u8 ); }
      256 => { break; } 
      _ => // 257 <= x && x <= 285 
      { 
        x -= 257;
        let length = MATCH_OFF[x] + inp.get_bits( MATCH_EXTRA[ x ] as usize );
        let dcode = inp.get_huff( 5 );
        let distance = DIST_OFF[dcode] + inp.get_bits( DIST_EXTRA[dcode] as usize );
        copy( out, distance, length );
      }
    }
  }
} // end do_fixed

// RFC 1951 constants.

pub static CLEN_ALPHABET : [u8; 19] = [ 16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15 ];

pub static MATCH_OFF : [usize; 30] = [ 3,4,5,6, 7,8,9,10, 11,13,15,17, 19,23,27,31, 35,43,51,59, 
  67,83,99,115,  131,163,195,227, 258, 0xffff ];

pub static MATCH_EXTRA : [u8; 29] = [ 0,0,0,0, 0,0,0,0, 1,1,1,1, 2,2,2,2, 3,3,3,3, 4,4,4,4, 5,5,5,5, 0 ];

pub static DIST_OFF : [usize; 30] = [ 1,2,3,4, 5,7,9,13, 17,25,33,49, 65,97,129,193, 257,385,513,769, 
  1025,1537,2049,3073, 4097,6145,8193,12289, 16385,24577 ];

pub static DIST_EXTRA : [u8; 30] = [ 0,0,0,0, 1,1,2,2, 3,3,4,4, 5,5,6,6, 7,7,8,8, 9,9,10,10, 11,11,12,12, 13,13 ];

