use crate::compress;
use crate::block::CLEN_ALPHABET;
use crate::block::MATCH_OFF;
use crate::block::MATCH_EXTRA;
use crate::block::DIST_OFF;
use crate::block::DIST_EXTRA;

pub fn test()
{
  let data = [1,2,3,4,1,2,3,4];
  let com = compress::compress( &data );
  let mut out: Vec<u8> = Vec::new();
  inflate( &com, &mut out );

  for i in 0..data.len()
  {
    if data[i] != out[i] { println!( "Failed at i={} {} {}", i, data[i], out[i] ); }
    assert_eq!( data[i], out[i] );
  }  
}

pub fn inflate( data: &[u8], out: &mut Vec<u8> )
{
  let mut inp = InpBitStream::new( &data );
  let _chk = inp.get_bits( 16 );
  loop
  {
    let last = inp.get_bit();
    let btype = inp.get_bits( 2 );
    // println!( "btype={}", btype );
    match btype
    {
      0 => { do_copy( &mut inp, out ); }
      1 => { do_fixed( &mut inp, out ); }
      2 => { do_dyn( &mut inp, out ); }
      _ => {}
    }

    if last != 0 { break; }
  }  
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
    if x <= 23 { x += 256; }
    else
    {
      x = ( x << 1 ) + inp.get_bit();
      if x <= 191 { x -= 48; }
      else if x <= 199 { x += 88; }
      else { x = ( x << 1 ) + inp.get_bit() - 256; }
    }

    if x < 256 { out.push( x as u8 ); }
    else if x == 256 { break; }
    else // 257 <= x && x <= 285
    {
      x -= 257;
      let length = MATCH_OFF[x] + inp.get_bits( MATCH_EXTRA[ x ] as usize );
      let dcode = inp.get_huff( 5 );
      let distance = DIST_OFF[dcode] + inp.get_bits( DIST_EXTRA[dcode] as usize );
      copy( out, distance, length );
    }
  }
} // end do_fixed

fn do_dyn( inp: &mut InpBitStream, out: &mut Vec<u8> )
{
  let n_lit_code = 257 + inp.get_bits(5);
  let n_dist_code = 1 + inp.get_bits(5);
  let n_len_code = 4 + inp.get_bits(4);
  // println!( "n..={} {} {}", n_lit_code, n_dist_code, n_len_code );

  let mut len = LenDecoder::new( inp, n_len_code );
  let lit = BitDecoder::new( &len.get_lengths( inp, n_lit_code ) );
  let dist = BitDecoder::new( &len.get_lengths( inp, n_dist_code ) );

  loop
  {
    let x = lit.decode( inp );

    if x < 256 { out.push( x as u8 ); }
    else if x == 256 { break; }
    else
    {
      let mc = x - 257;
      let length = MATCH_OFF[ mc ] + inp.get_bits( MATCH_EXTRA[ mc ] as usize );
      let dc = dist.decode( inp );
      let distance = DIST_OFF[ dc ] + inp.get_bits( DIST_EXTRA[ dc ] as usize );
      // println!( "Copy at {} length={} distance={}", out.len(), length, distance  );
      copy( out, distance, length ); 
    }
  }
} // end do_dyn

fn copy( out: &mut Vec<u8>, distance: usize, mut length: usize )
{
  let mut i = out.len();
  while length > 0
  {
    out.push( out[ i - distance ] );
    i += 1;
    length -= 1;
  }
}

struct LenDecoder
{
  plenc: usize,
  rep: usize,
  bd: BitDecoder,
}

impl LenDecoder
{
  fn new( inp: &mut InpBitStream, n_len_code: usize ) -> LenDecoder
  {
    let mut clen_len:[ usize; 19 ] = [0; 19 ];

    for i in 0..n_len_code { clen_len[ CLEN_ALPHABET[i] as usize ] = inp.get_bits(3); }

    LenDecoder
    {
      plenc: 0, rep:0, bd: BitDecoder::new( &clen_len )
    }
  }

  fn get_lengths( &mut self, inp: &mut InpBitStream, n: usize ) -> Vec<usize> // Per RFC1931 page 13.
  {
    let mut la: Vec<usize> = vec![ 0; n ];

    let mut i = 0;
    while self.rep > 0 { la[i] = self.plenc; i += 1; self.rep -= 1; }
    while i < n
    { 
      let lenc = self.bd.decode( inp );

      if lenc < 16 { la[i] = lenc; i += 1; self.plenc = lenc; }
      else 
      {
        if lenc == 16 { self.rep = 3 + inp.get_bits(2); }
        else if lenc == 17 { self.rep = 3 + inp.get_bits(3); self.plenc=0; }
        else if lenc == 18 { self.rep = 11 + inp.get_bits(7); self.plenc=0; } 
        while i < n && self.rep > 0 { la[i] = self.plenc; i += 1; self.rep -= 1; }
      }
      // println!( "i={} n={}", i, n );
    }
    la
  } // end get_lengths
}

struct BitDecoder
{
  root: usize,
  left: Vec<usize>,
  right: Vec<usize>,
}

const NIL : usize = 0xffffffff;

impl BitDecoder
{
  fn new( nbits: &[usize] ) -> BitDecoder
  {
    let ncode = nbits.len();
    let mut result =
    BitDecoder
    {
      root: NIL,
      left: Vec::with_capacity( ncode ),
      right: Vec::with_capacity( ncode )
    };
    result.make_tree( ncode, nbits );
    result
  }

  fn make_tree( &mut self, ncode: usize, nbits: &[usize] )
  {
    // Code below is from rfc1951 page 7

    let mut max_bits : usize = 0; 
    for i in 0..ncode { if nbits[i] > max_bits { max_bits = nbits[i]; } }
    // println!( "max_bits={}", max_bits );

    let mut bl_count : Vec<usize> = vec![ 0; max_bits + 1 ];
    // println!( "bl_count.len={}", bl_count.len() );

    for i in 0..ncode { bl_count[ nbits[i] ] += 1; }

    let mut next_code : Vec<usize> = vec![ 0; max_bits + 1 ];
    let mut code = 0; 
    bl_count[0] = 0;

    for i in 0..max_bits
    {
      code = ( code + bl_count[i] ) << 1;
      next_code[ i + 1 ] = code;
    }

    let mut tree_code : Vec<usize> = vec![ 0; ncode ];
    for i in 0..ncode
    {
      let len = nbits[ i ];
      if len != 0
      {
        tree_code[ i ] = next_code[ len ];
        next_code[ len ] += 1;
      }
    }

    for i in 0..ncode
    {
      if nbits[i] > 0
      {
        // println!( "insert i={} nbits={} code={}", i, nbits[i], tree_code[i] );
        self.root = self.insert( self.root, i, nbits[i], tree_code[i] );
      }
    }    
  }

  fn insert( &mut self, mut x: usize, value: usize, len: usize, code: usize ) -> usize
  {
    if x == NIL 
    {
      x = self.left.len();
      self.left.push( NIL );
      self.right.push( NIL );
    }

    if len == 0 
    {
      self.right[x] = value;
    }
    else if ( code >> ( len - 1 ) & 1 ) == 0
    {
      self.left[x] = self.insert( self.left[x], value, len-1, code );
    }
    else 
    {
      self.right[x] = self.insert( self.right[x], value, len-1, code ); 
    }
    return x;
  }

  // A more efficient implementation would use a lookup table after fetching several bits rather than a single bit.
  fn decode( &self, input: &mut InpBitStream ) -> usize
  {
    let mut n = 0;
    while self.left[ n ] != NIL
    {
      n = if input.get_bit() == 0 { self.left[ n ] } else { self.right[ n ] }
    }
    let result = self.right[ n ];

    // println!( "decode result={}", result );

    result
  }

}

struct InpBitStream<'a>
{
  data: &'a [u8],
  pos: usize,
  buf: usize,
}

impl <'a> InpBitStream<'a>
{
  fn new( data: &'a [u8] ) -> InpBitStream
  {
    InpBitStream
    {
      data,
      pos: 0,
      buf: 1,
    }
  } 

  fn get_bit( &mut self ) -> usize
  {
    if self.buf == 1
    {
      self.buf = self.data[ self.pos ] as usize | 256;
      self.pos += 1;
    }
    let result = self.buf & 1;
    self.buf >>= 1;
    result
  }

  /// Get bits, least sig bit first
  fn get_bits( &mut self, n: usize ) -> usize
  { 
    let mut result = 0; 
    for i in 0..n
    {
      result |= self.get_bit() << i; 
    }
    result
  }

  //// Get bits, most sig bit first
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
    self.buf = 1;
  }
}