// RFC 1951 constants.

pub static CLEN_ALPHABET : [u8; 19] = [ 16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15 ];

pub static MATCH_OFF : [usize; 30] = [ 3,4,5,6, 7,8,9,10, 11,13,15,17, 19,23,27,31, 35,43,51,59, 
  67,83,99,115,  131,163,195,227, 258, 0xffff ];

pub static MATCH_EXTRA : [u8; 29] = [ 0,0,0,0, 0,0,0,0, 1,1,1,1, 2,2,2,2, 3,3,3,3, 4,4,4,4, 5,5,5,5, 0 ];

pub static DIST_OFF : [usize; 30] = [ 1,2,3,4, 5,7,9,13, 17,25,33,49, 65,97,129,193, 257,385,513,769, 
  1025,1537,2049,3073, 4097,6145,8193,12289, 16385,24577 ];

pub static DIST_EXTRA : [u8; 30] = [ 0,0,0,0, 1,1,2,2, 3,3,4,4, 5,5,6,6, 7,7,8,8, 9,9,10,10, 11,11,12,12, 13,13 ];

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
      0 => { do_copy( &mut inp, &mut out ); }
      1 => { do_fixed( &mut inp, &mut out ); }
      2 => { do_dyn( &mut inp, &mut out ); }
      _ => { }
    }

    if last != 0 { break; }
  }  
  out
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

fn do_dyn( inp: &mut InpBitStream, out: &mut Vec<u8> )
{
  let n_lit_code = 257 + inp.get_bits(5);
  let n_dist_code = 1 + inp.get_bits(5);
  let n_len_code = 4 + inp.get_bits(4);

  let mut len = LenDecoder::new( inp, n_len_code );
  let lit = BitDecoder::new( &len.get_lengths( inp, n_lit_code ) );
  let dist = BitDecoder::new( &len.get_lengths( inp, n_dist_code ) );

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
    let mut result = BitDecoder { root: NIL, left: Vec::with_capacity( ncode ), right: Vec::with_capacity( ncode ) };
    result.make_tree( ncode, nbits );
    result
  }

  fn make_tree( &mut self, ncode: usize, nbits: &[usize] )
  {
    // Code below is from rfc1951 page 7

    let mut max_bits : usize = 0; 
    for bits in nbits { if *bits > max_bits { max_bits = *bits; } }

    let mut bl_count : Vec<usize> = vec![ 0; max_bits + 1 ];

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
    } else if ( code >> ( len - 1 ) & 1 ) == 0 {
      self.left[x] = self.insert( self.left[x], value, len-1, code );
    } else {
      self.right[x] = self.insert( self.right[x], value, len-1, code ); 
    }
    x
  }

  // The function result depends on the next few bits of input.
  // A more efficient implementation would fetch several input bits and then use a lookup table.
  fn decode( &self, input: &mut InpBitStream ) -> usize
  {
    let mut n = 0;
    while self.left[ n ] != NIL
    {
      n = if input.get_bit() == 0 { self.left[ n ] } else { self.right[ n ] }
    }
    self.right[ n ]
  }
} // end impl BitDecoder

struct InpBitStream<'a>
{
  data: &'a [u8],
  pos: usize,
  buf: usize
}

impl <'a> InpBitStream<'a>
{
  fn new( data: &'a [u8] ) -> InpBitStream
  {
    InpBitStream { data, pos: 0, buf: 1 }
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
    // Read the array of 3-bit code lengths ( used to encode ac4tual code lengths ) from input.
    let mut clen_len:[ usize; 19 ] = [0; 19 ];
    for i in 0..n_len_code { clen_len[ CLEN_ALPHABET[i] as usize ] = inp.get_bits(3); }

    LenDecoder { plenc: 0, rep:0, bd: BitDecoder::new( &clen_len ) }
  }

  // Per RFC1931 page 13, get array of code lengths.
  fn get_lengths( &mut self, inp: &mut InpBitStream, n: usize ) -> Vec<usize>
  {
    let mut result: Vec<usize> = vec![ 0; n ];

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
    result
  } // end get_lengths
} // end impl LenDecder
