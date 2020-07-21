/// RFC 1951 inflate ( de-compress ).

pub fn inflate( data: &[u8] ) -> Vec<u8>
{
  let mut input = InpBitStream::new( &data );
  let mut output = Vec::new();
  let _check_sum = input.get_bits( 16 );
  loop
  {
    let last_block = input.get_bit();
    let block_type = input.get_bits( 2 );
    match block_type
    {
      2 => { do_dyn( &mut input, &mut output ); }
      1 => { do_fixed( &mut input, &mut output ); }
      0 => { do_copy( &mut input, &mut output ); }
      _ => { }
    }
    if last_block != 0 { break; }
  }  
  output
}

fn do_dyn( input: &mut InpBitStream, output: &mut Vec<u8> )
{
  let n_lit = 257 + input.get_bits( 5 );
  let n_dist = 1 + input.get_bits( 5 );
  let n_len = 4 + input.get_bits( 4 );

  let mut len = LenDecoder::new( n_len, input );
  let lit = len.get_decoder( n_lit, input );
  let dist = len.get_decoder( n_dist, input ); 

  loop
  {
    let x = lit.decode( input );
    match x
    {
      0..=255 => { output.push( x as u8 ); }
      256 =>  { break; } 
      _ =>
      {
        let mc = x - 257;
        let length = MATCH_OFF[ mc ] + input.get_bits( MATCH_EXTRA[ mc ] as usize );
        let dc = dist.decode( input );
        let distance = DIST_OFF[ dc ] + input.get_bits( DIST_EXTRA[ dc ] as usize );
        copy( output, distance, length ); 
      }
    }
  }
} // end do_dyn

fn copy( output: &mut Vec<u8>, distance: usize, mut length: usize )
{
  let mut i = output.len() - distance;
  while length > 0
  {
    output.push( output[ i ] );
    i += 1;
    length -= 1;
  }
}

/// Decode length-limited Huffman codes.
struct BitDecoder
{
  nsym: usize, // The number of symbols.
  nbits: Vec<u8>, // The length in bits of the code that represents each symbol.
  maxbits: usize, // The length in bits of the longest code.
  peekbits: usize, // The bit lookup length for the first lookup.
  lookup: Vec<usize> // The lookup table used to lookup up a symbol from a code.
}

impl BitDecoder
{
  fn new( nsym: usize ) -> BitDecoder
  {
    BitDecoder 
    { 
      nsym,
      nbits: vec![0; nsym],
      maxbits: 0,
      peekbits: 0,
      lookup: Vec::new()
    }
  }

  // Read an encoded symbol from the input bit-stream.
  // To keep the lookup table small codes longer than 8 bits are looked up in two peeks.
  fn decode( &self, input: &mut InpBitStream ) -> usize
  {
    let mut sym = self.lookup[ input.peek( self.peekbits ) ];
    if sym >= self.nsym
    {
      sym = self.lookup[ sym - self.nsym + ( input.peek( self.maxbits ) >> self.peekbits ) ];
    }  
    input.advance( self.nbits[ sym ] as usize );
    sym
  }

  fn init( &mut self )
  {
    let nsym = self.nsym;

    let mut max_bits : usize = 0; 
    for bp in &self.nbits 
    { 
      let bits = *bp as usize;
      if bits > max_bits { max_bits = bits; } 
    }

    self.maxbits = max_bits;
    self.peekbits = if max_bits > 8 { 8 } else { max_bits };
    self.lookup.resize( 1 << self.peekbits, 0 );

    // Code below is from rfc1951 page 7.

    // bl_count is the number of codes of length N, N >= 1.
    let mut bl_count : Vec<usize> = vec![ 0; max_bits + 1 ];

    for sym in 0..nsym { bl_count[ self.nbits[ sym ] as usize ] += 1; }

    let mut next_code : Vec<usize> = vec![ 0; max_bits + 1 ];
    let mut code = 0; 
    bl_count[ 0 ] = 0;

    for i in 0..max_bits
    {
      code = ( code + bl_count[ i ] ) << 1;
      next_code[ i + 1 ] = code;
    }

    for sym in 0..nsym
    {
      let length = self.nbits[ sym ] as usize;
      if length != 0
      {
        self.setup_code( sym, length, next_code[ length ] );
        next_code[ length ] += 1;
      }
    }
  }

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
        self.lookup[ kr ] = self.nsym + base;
      } else {
        base -= self.nsym;
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
  data: &'a [u8], // Input data.
  pos: usize, // Position in input data.
  buf: usize, // Bit buffer.
  got: usize, // Number of bits in buffer.
}

impl <'a> InpBitStream<'a>
{
  fn new( data: &'a [u8] ) -> InpBitStream
  {
    InpBitStream { data, pos: 0, buf: 1, got: 0 }
  } 

  // Get n bits of input ( but do not advance ).
  fn peek( &mut self, n: usize ) -> usize
  {
    while self.got < n
    {
      if self.pos < self.data.len() 
      {
        self.buf |= ( self.data[ self.pos ] as usize ) << self.got;
      }
      self.pos += 1;
      self.got += 8;
    }
    self.buf & ( ( 1 << n ) - 1 )
  }

  // Advance n bits.
  fn advance( &mut self, n:usize )
  { 
    self.buf >>= n;
    self.got -= n;
  }

  // Get a single bit.
  fn get_bit( &mut self ) -> usize
  {
    if self.got == 0 { self.peek( 1 ); }
    let result = self.buf & 1;
    self.advance( 1 );
    result
  }

  // Get n bits of input.
  fn get_bits( &mut self, n: usize ) -> usize
  { 
    let result = self.peek( n );
    self.advance( n );
    result
  }

  // Get n bits of input, reversed.
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

  // Discard any buffered bits.
  fn clear_bits( &mut self )
  {
    // Note: this might work right if peeking more than 8 bits.
    self.got = 0;
  }
} //  end impl InpBitStream

/// Decode code lengths, per RFC 1951 page 13.
struct LenDecoder
{
  plenc: u8, // previous length code ( which can be repeated )
  rep: usize,   // repeat
  bd: BitDecoder
}

/// Decodes an array of lengths. There are special codes for repeats, and repeats of zeros.
impl LenDecoder
{
  fn new(  n_len: usize, input: &mut InpBitStream ) -> LenDecoder
  {
    let mut result = LenDecoder { plenc: 0, rep:0, bd: BitDecoder::new( 19 ) };

    // Read the array of 3-bit code lengths from input.
    for i in CLEN_ALPHABET.iter().take( n_len )
    { 
      result.bd.nbits[ *i as usize ] = input.get_bits(3) as u8; 
    }
    result.bd.init();
    result
  }

  fn get_decoder( &mut self, nsym: usize, input: &mut InpBitStream ) -> BitDecoder
  {
    let mut result = BitDecoder::new( nsym );
    let nbits = &mut result.nbits;
    let mut i = 0;
    while self.rep > 0 { nbits[i] = self.plenc; i += 1; self.rep -= 1; }
    while i < nsym
    { 
      let lenc = self.bd.decode( input ) as u8;
      if lenc < 16 
      {
        nbits[i] = lenc; 
        i += 1; 
        self.plenc = lenc; 
      } else {
        if lenc == 16 { self.rep = 3 + input.get_bits(2); }
        else if lenc == 17 { self.rep = 3 + input.get_bits(3); self.plenc=0; }
        else if lenc == 18 { self.rep = 11 + input.get_bits(7); self.plenc=0; } 
        while i < nsym && self.rep > 0 { nbits[i] = self.plenc; i += 1; self.rep -= 1; }
      }
    }
    result.init();
    result
  }
} // end impl LenDecoder

/// Reverse a string of n bits.
pub fn reverse( mut x:usize, mut n: usize ) -> usize
{ 
  let mut result: usize = 0; 
  while n > 0
  {
    result = ( result << 1 ) | ( x & 1 ); 
    x >>= 1; 
    n -= 1;
  } 
  result
} 

fn do_copy( input: &mut InpBitStream, output: &mut Vec<u8> )
{
  input.clear_bits(); // Discard any bits in the input buffer
  let mut n = input.get_bits( 16 );
  let _n1 = input.get_bits( 16 );
  while n > 0 { output.push( input.data[ input.pos ] ); n -= 1; input.pos += 1; }
}

fn do_fixed( input: &mut InpBitStream, output: &mut Vec<u8> ) // RFC1951 page 12.
{
  loop
  {
    // 0 to 23 ( 7 bits ) => 256 - 279; 48 - 191 ( 8 bits ) => 0 - 143; 
    // 192 - 199 ( 8 bits ) => 280 - 287; 400..511 ( 9 bits ) => 144 - 255
    let mut x = input.get_huff( 7 ); // Could be optimised. 
    if x <= 23 
    { 
      x += 256; 
    } else {
      x = ( x << 1 ) + input.get_bit();
      if x <= 191 { x -= 48; }
      else if x <= 199 { x += 88; }
      else { x = ( x << 1 ) + input.get_bit() - 256; }
    }

    match x
    {
      0..=255 => { output.push( x as u8 ); }
      256 => { break; } 
      _ => // 257 <= x && x <= 285 
      { 
        x -= 257;
        let length = MATCH_OFF[x] + input.get_bits( MATCH_EXTRA[ x ] as usize );
        let dcode = input.get_huff( 5 );
        let distance = DIST_OFF[dcode] + input.get_bits( DIST_EXTRA[dcode] as usize );
        copy( output, distance, length );
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

