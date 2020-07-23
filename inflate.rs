/// RFC 1951 inflate ( de-compress ).

pub fn inflate( data: &[u8] ) -> Vec<u8>
{
  let mut input = InputBitStream::new( &data );
  let mut output = Vec::new();
  let _flags = input.get_bits( 16 );
  loop
  {
    let last_block = input.get_bit();
    let block_type = input.get_bits( 2 );
    match block_type
    {
      2 => dyn_block( &mut input, &mut output ),
      1 => fixed_block( &mut input, &mut output ),
      0 => copy_block( &mut input, &mut output ),
      _ => ()
    }
    if last_block != 0 { break; }
  }  
  // Check the checksum.
  input.pad( 8 );
  let check_sum = input.get_bits(32) as u32;
  if crate::compress::adler32( &output ) != check_sum { panic!( "Bad checksum" ) }
  output
}

/// Decode block encoded with dynamic Huffman codes.
fn dyn_block( input: &mut InputBitStream, output: &mut Vec<u8> )
{
  let n_lit = 257 + input.get_bits( 5 );
  let n_dist = 1 + input.get_bits( 5 );
  let n_len = 4 + input.get_bits( 4 );

  // The lengths of the main Huffman codes (lit,dist) are themselves decoded by LenDecoder.
  let mut len = LenDecoder::new( n_len, input );
  let lit : BitDecoder = len.get_decoder( n_lit, input );
  let dist : BitDecoder = len.get_decoder( n_dist, input ); 

  loop
  {
    let x : usize = lit.decode( input );
    match x
    {
      0..=255 => output.push( x as u8 ),
      256 => break,
      _ => // LZ77 match code - replicate earlier output.
      {
        let mc = x - 257;
        let length = MATCH_OFF[ mc ] as usize + input.get_bits( MATCH_EXTRA[ mc ] as usize );
        let dc = dist.decode( input );
        let distance = DIST_OFF[ dc ] as usize + input.get_bits( DIST_EXTRA[ dc ] as usize );
        copy( output, distance, length ); 
      }
    }
  }
} // end do_dyn

/// Copy length bytes from output ( at specified distance ) to output.
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
// For speed, a lookup table is used to compute symbols from the variable length codes ( rather than reading single bits ).
// To keep the lookup table small, codes longer than PEEK bits are looked up in two operations.
struct BitDecoder
{
  nsym: usize, // The number of symbols.
  bits: Vec<u8>, // The length in bits of the code that represents each symbol.
  maxbits: usize, // The length in bits of the longest code.
  peekbits: usize, // The bit length for the first lookup ( not greater than PEEK ).
  lookup: Vec<usize> // The table used to look up a symbol from a code.
}

/// Maximum number of bits for first lookup.
const PEEK : usize = 8; 

impl BitDecoder
{
  fn new( nsym: usize ) -> BitDecoder
  {
    BitDecoder 
    { 
      nsym,
      bits: vec![0; nsym],
      maxbits: 0,
      peekbits: 0,
      lookup: Vec::new()
    }
  }

  /// The main function : get a decoded symbol from the input bit stream.
  /// Codes of up to PEEK bits are looked up in a single operation.
  /// Codes of more than PEEK bits are looked up in two steps.
  fn decode( &self, input: &mut InputBitStream ) -> usize
  {
    let mut sym = self.lookup[ input.peek( self.peekbits ) ];
    if sym >= self.nsym
    {
      sym = self.lookup[ sym - self.nsym + ( input.peek( self.maxbits ) >> self.peekbits ) ];
    }  
    input.advance( self.bits[ sym ] as usize );
    sym
  }

  fn init_lookup( &mut self )
  {
    let mut max_bits : usize = 0; 
    for bp in &self.bits 
    { 
      let bits = *bp as usize;
      if bits > max_bits { max_bits = bits; } 
    }

    self.maxbits = max_bits;
    self.peekbits = if max_bits > PEEK { PEEK } else { max_bits };
    self.lookup.resize( 1 << self.peekbits, 0 );

    // Code below is from rfc1951 page 7.

    // bl_count is the number of codes of length N, N >= 1.
    let mut bl_count : Vec<usize> = vec![ 0; max_bits + 1 ];

    for sym in 0..self.nsym { bl_count[ self.bits[ sym ] as usize ] += 1; }

    let mut next_code : Vec<usize> = vec![ 0; max_bits + 1 ];
    let mut code = 0; 
    bl_count[ 0 ] = 0;

    for i in 0..max_bits
    {
      code = ( code + bl_count[ i ] ) << 1;
      next_code[ i + 1 ] = code;
    }

    for sym in 0..self.nsym
    {
      let length = self.bits[ sym ] as usize;
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
      code <<= diff;
      for i in code..code + (1 << diff)
      {
        // lookup index is reversed to match InputBitStream::peek
        self.lookup[ reverse( i, self.peekbits ) ] = sym;
      }
    } else { // Secondary lookup required      
      let peekbits2 = self.maxbits - self.peekbits;

      // Split code into peekbits portion ( key ) and remainder ( code).
      let diff1 = len - self.peekbits;
      let key = reverse( code >> diff1, self.peekbits );
      code &= ( 1 << diff1 ) - 1;

      // Get the base for the secondary lookup.
      let mut base = self.lookup[ key ];
      if base == 0 // Secondary lookup not yet allocated for this key.
      {
        base = self.lookup.len();
        self.lookup.resize( base + ( 1 << peekbits2 ), 0 );
        self.lookup[ key ] = self.nsym + base;
      } else {
        base -= self.nsym;
      }

      // Set the secondary lookup values.
      let diff = self.maxbits - len;
      code <<= diff;
      for i in code..code + (1<<diff)
      { 
        self.lookup[ base + reverse( i, peekbits2 ) ] = sym;
      }
    }    
  }
} // end impl BitDecoder

/// Decodes an array of lengths, returning a new BitDecoder.  
/// There are special codes for repeats, and repeats of zeros, per RFC 1951 page 13.
struct LenDecoder
{
  plenc: u8, // previous length code ( which can be repeated )
  rep: usize,   // repeat
  bd: BitDecoder
}

impl LenDecoder
{
  fn new(  n_len: usize, input: &mut InputBitStream ) -> LenDecoder
  {
    let mut result = LenDecoder { plenc: 0, rep:0, bd: BitDecoder::new( 19 ) };

    // Read the array of 3-bit code lengths (used to encode the main code lengths ) from input.
    for i in CLEN_ALPHABET.iter().take( n_len )
    { 
      result.bd.bits[ *i as usize ] = input.get_bits(3) as u8; 
    }
    result.bd.init_lookup();
    result
  }

  fn get_decoder( &mut self, nsym: usize, input: &mut InputBitStream ) -> BitDecoder
  {
    let mut result = BitDecoder::new( nsym );
    let bits = &mut result.bits;
    let mut i = 0;
    while self.rep > 0 { bits[ i ] = self.plenc; i += 1; self.rep -= 1; }
    while i < nsym
    { 
      let lenc = self.bd.decode( input ) as u8;
      if lenc < 16 
      {
        bits[ i ] = lenc; 
        i += 1; 
        self.plenc = lenc; 
      } else {
        if lenc == 16 { self.rep = 3 + input.get_bits(2); }
        else if lenc == 17 { self.rep = 3 + input.get_bits(3); self.plenc=0; }
        else if lenc == 18 { self.rep = 11 + input.get_bits(7); self.plenc=0; } 
        while i < nsym && self.rep > 0 { bits[ i ] = self.plenc; i += 1; self.rep -= 1; }
      }
    }
    result.init_lookup();
    result
  }
} // end impl LenDecoder

/// For reading bits from input array of bytes.
struct InputBitStream<'a>
{
  data: &'a [u8], // Input data.
  pos: usize, // Position in input data.
  buf: usize, // Bit buffer.
  got: usize, // Number of bits in buffer.
}

impl <'a> InputBitStream<'a>
{
  fn new( data: &'a [u8] ) -> InputBitStream
  {
    InputBitStream { data, pos: 0, buf: 1, got: 0 }
  } 

  // Get n bits of input ( but do not advance ).
  fn peek( &mut self, n: usize ) -> usize
  {
    while self.got < n
    {
      // Not necessary to check index, considering adler32 checksum is 32 bits.
      self.buf |= ( self.data[ self.pos ] as usize ) << self.got;
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

  // Move to n-bit boundary ( n a power of 2 ).
  fn pad( &mut self, n: usize )
  {  
    self.got -= self.got % n;
  }
} // end impl InputBitStream

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

/// Copy uncompressed block to output.
fn copy_block( input: &mut InputBitStream, output: &mut Vec<u8> )
{
  input.pad( 8 ); // Move to 8-bit boundary.
  let mut n = input.get_bits( 16 );
  let _n1 = input.get_bits( 16 );
  while n > 0 { output.push( input.data[ input.pos ] ); n -= 1; input.pos += 1; }
}

/// Decode block encoded with fixed (pre-defined) Huffman codes.
fn fixed_block( input: &mut InputBitStream, output: &mut Vec<u8> ) // RFC1951 page 12.
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
        let length = MATCH_OFF[x] as usize + input.get_bits( MATCH_EXTRA[ x ] as usize );
        let dcode = input.get_huff( 5 );
        let distance = DIST_OFF[dcode] as usize + input.get_bits( DIST_EXTRA[dcode] as usize );
        copy( output, distance, length );
      }
    }
  }
} // end fixed_block

// RFC 1951 constants.

pub static CLEN_ALPHABET : [u8; 19] = [ 16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15 ];

pub static MATCH_OFF : [u16; 30] = [ 3,4,5,6, 7,8,9,10, 11,13,15,17, 19,23,27,31, 35,43,51,59, 
  67,83,99,115,  131,163,195,227, 258, 0xffff ];

pub static MATCH_EXTRA : [u8; 29] = [ 0,0,0,0, 0,0,0,0, 1,1,1,1, 2,2,2,2, 3,3,3,3, 4,4,4,4, 5,5,5,5, 0 ];

pub static DIST_OFF : [u16; 30] = [ 1,2,3,4, 5,7,9,13, 17,25,33,49, 65,97,129,193, 257,385,513,769, 
  1025,1537,2049,3073, 4097,6145,8193,12289, 16385,24577 ];

pub static DIST_EXTRA : [u8; 30] = [ 0,0,0,0, 1,1,2,2, 3,3,4,4, 5,5,6,6, 7,7,8,8, 9,9,10,10, 11,11,12,12, 13,13 ];

