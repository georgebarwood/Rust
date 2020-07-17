use crate::col::Heap;

/// RFC 1951 length-limited Huffman coding.
pub struct BitCoder
{
  pub symbols: usize,  // Number of symbols to be encoded (input/output).
  pub used: Vec<u32>,  // Number of times each symbol is used in the block being encoded ( input ).
  pub bits: Vec<u8>,   // Number of bits used to encode each symbol ( output ).
  pub code: Vec<u16>,  // Code for each symbol (output).

  lim_bits: usize,  // Limit on code length ( 15 or 7 for RFC 1951 ).
  max_bits: usize,  // Maximum code length.
  left: Vec<u16>, right: Vec<u16>, // Tree storage.
}

impl BitCoder
{
  pub fn new( lim_bits: usize, symbols: usize ) -> BitCoder
  {
    BitCoder
    { 
      symbols,
      lim_bits, 
      max_bits: 0,
      used:  vec![0;symbols],
      bits:  vec![0;symbols],
      left:  vec![0;symbols],
      right: vec![0;symbols],
      code:  Vec::with_capacity( symbols ),
    }
  }

  pub fn compute_bits( &mut self ) // Compute bits from used.
  {
    // First try to compute a Huffman code.
    // Most of the time this succeeds, but sometime lim_bits is exceeeded in which case package_merge is used.

    // Tree nodes are encoded in a u64 using 32 bits for used count, 8 bits for the tree depth, 16 bits for the id.
    // Constants for accessing the bitfields.
    const USEDBITS : u8 = 32;
    const DEPTHBITS : u8 = 8;
    const IDBITS : u8 = 16;

    const USEDMASK : u64 = ( ( 1 << USEDBITS ) - 1 ) << ( IDBITS + DEPTHBITS );
    const DEPTHMASK : u64 = ( ( 1 << DEPTHBITS ) - 1 ) << IDBITS;
    const DEPTHONE : u64 = 1 << IDBITS;
    const IDMASK : u64 = ( 1 << IDBITS ) - 1;

    // First compute the number of bits to encode each symbol (self.bits), using a Heap.
    let mut heap = Heap::<u64>::new( self.symbols as usize );

    // Add the leaf nodes to the heap.
    for id in 0..self.symbols
    {
      let used = self.used[ id ];
      if used > 0 
      { 
        heap.add( ( used as u64 ) << ( IDBITS + DEPTHBITS ) | id as u64 );
      }
    }
    heap.make();

    // Construct the binary (non-leaf) nodes of the tree.
    let non_zero : usize = heap.count();
   
    match non_zero
    {
      0 => {}
      1 =>
      { 
        self.get_bits( ( heap.remove() & IDMASK ) as usize, 1 );
        self.max_bits = 1;
      } 
      _ =>
      {
        let mut node = 0;

        loop // Keep pairing the lowest frequency (least used) tree nodes.
        {
          let left = heap.remove(); 
          self.left[ node ] = ( left & IDMASK ) as u16;

          let right = heap.remove(); 
          self.right[ node ] = ( right & IDMASK ) as u16;

          // Extract depth of left and right nodes ( still shifted though ).
          let depth_left = left & DEPTHMASK;
          let depth_right = right & DEPTHMASK; 

          // New node depth is 1 + larger of depth_left and depth_right.
          let depth = DEPTHONE + std::cmp::max(depth_left,depth_right);

          // Add the new tree node to the heap, as above, Used | Depth | Id
          heap.insert( ( left + right ) & USEDMASK | depth | ( self.symbols + node ) as u64 );

          node += 1;

          if heap.count() < 2 { break }
        }
        
        let root = ( heap.remove() & ( DEPTHMASK | IDMASK ) ) as usize;
        self.max_bits = root >> IDBITS;
        if self.max_bits <= self.lim_bits
        {
          self.get_bits( root & IDMASK as usize, 0 );
        } else {
          self.max_bits = self.lim_bits;
          self.package_merge( non_zero );
        }
      }
    }

    // Reduce symbol count if there are unused trailing symbols.
    while self.symbols > 0 && self.bits[ self.symbols - 1 ] == 0
    { 
      self.symbols -= 1; 
    }
/*
    println!( "computed bits" );
    for i in 0..self.symbols
    {
      if self.bits[i] > 0
      {
        println!( "symbol={} used={} bits={}", i, self.used[i], self.bits[i] );
      }
    }
*/
  }

  fn get_bits( &mut self, mut tree_node: usize, mut depth:u8 )
  {
    // Walk the tree reading off the number of bits to encode each symbol ( which is depth of tree ).
   
    if tree_node < self.symbols // node is a leaf.
    {
      self.bits[ tree_node ] = depth;
    } else {
      tree_node -= self.symbols;
      depth += 1;
      self.get_bits( self.left[ tree_node ] as usize, depth );
      self.get_bits( self.right[ tree_node ] as usize, depth );
    }
  }

  fn package_merge( &mut self, non_zero : usize )
  {
    // Tree nodes are encoded in a ulong using 16 bits for the id, 32 bits for Used.
    const IDBITS : i32 = 16;
    const IDMASK : u64 = ( 1 << IDBITS ) - 1;
    const USEDBITS : i32 = 32;
    const USEDMASK : u64 = ( ( 1 << USEDBITS ) - 1 ) << IDBITS;

    let tree_size = self.symbols * self.lim_bits;

    // Tree storage.
    self.left = vec![ 0; tree_size ];
    self.right = vec![ 0; tree_size ];

    // First create the leaf nodes for the tree and sort.
    let mut leaves : Vec<u64> = Vec::with_capacity( non_zero );

    for i in 0..self.symbols
    {
      let used = self.used[ i ];
      if used != 0 
      {
        leaves.push( (used as u64) << IDBITS | i as u64 );
      }
    }
    leaves.sort();

    let mut merged = Vec::<u64>::with_capacity( self.symbols );
    let mut next = Vec::<u64>::with_capacity( self.symbols );

    let mut package : usize = self.symbols; // Allocator for package (tree node) ids.

    for _i in 0..self.lim_bits
    {
      let mut lix = 0; // Index into leaves.
      let mut mix = 0; // Index into merged.
      let llen = leaves.len();
      let mlen = merged.len();
      let mut total = ( llen + mlen ) / 2;
      while total > 0
      {
        // Compute left.
        let mut left : u64;
        if mix < mlen
        {
          left = merged[ mix ];
          if lix < llen
          {
            let leaf = leaves[ lix ];
            if left < leaf { mix += 1; }
            else { left = leaf; lix += 1; }
          }
          else { mix += 1; }
        }
        else { left = leaves[ lix ]; lix += 1; }

        // Compute right.
        let mut right : u64;
        if mix < mlen
        {
          right = merged[ mix ];
          if lix < llen
          {
            let leaf = leaves[ lix ];
            if right < leaf { mix += 1; }
            else { right = leaf; lix += 1; }
          }
          else { mix += 1; }
        }
        else { right = leaves[ lix ]; lix += 1; }

        // Package left and right.  
        self.left[ package ] = ( left & IDMASK ) as u16;
        self.right[ package ] = ( right & IDMASK ) as u16;
        next.push( ( left + right ) & USEDMASK | package as u64 );        
        package += 1;
        total -= 1;
      }

      // Swap merged and next.
      std::mem::swap( &mut merged, &mut next );
      next.clear();
    }

    // Calculate the number of bits to encode each symbol.
    for node in merged
    {
      self.merge_get_bits( ( node & IDMASK ) as usize );
    }
  }

  fn merge_get_bits( &mut self, node : usize )
  {
    if node < self.symbols
    {
      self.bits[ node ] += 1;
    } else {
      self.merge_get_bits( self.left[ node ] as usize );
      self.merge_get_bits( self.right[ node ] as usize );
    }
  }

  pub fn total( &mut self ) -> usize
  {
    let mut result = 0;
    for i  in 0..self.symbols
    {
      result += self.used[ i ] as usize * self.bits[ i ] as usize;
    }
    result
  }

  pub fn compute_codes( &mut self )
  {
    // Code below is from RFC 1951 page 7.

    // bl_count[N] is the number of symbols encoded with N bits.
    let mut bl_count : Vec<u16> = vec![ 0; self.max_bits + 1 ];
    for sym in 0..self.symbols
    {
      bl_count[ self.bits[ sym ] as usize ] += 1; 
    }

    // Find the numerical value of the smallest code for each code length.
    let mut next_code : Vec<u16> = Vec::with_capacity( self.max_bits + 1 );
    let mut code : u16 = 0; 
    bl_count[ 0 ] = 0;
    next_code.push( 0 );
    for bc in bl_count
    {
      code = ( code + bc ) << 1;
      next_code.push( code );
    }

    // Calculate the result.
    for sym in 0..self.symbols
    {
      let length = self.bits[ sym ] as usize;      
      self.code.push( reverse( next_code[ length ], length ) as u16 );
      next_code[ length ] += 1;
    }
  }

} // end impl BitCoder

/// RFC 1951 encoding of lengths.
pub struct LenCoder
{
  pub bc: BitCoder,
  pub length_pass: u8, 
  previous_length: usize, zero_run: usize, repeat: usize,
}

impl LenCoder
{
  pub fn new( limit:usize, symbols:usize ) -> LenCoder
  {
    LenCoder
    {
      bc: BitCoder::new( limit, symbols ),
      length_pass: 0,
      previous_length: 0,
      zero_run: 0,
      repeat: 0,
    }
  }

  // Run length encoding of code lengths - RFC 1951, page 13.

  pub fn encode_lengths( &mut self, is_lit: bool, count: usize, lengths: &[u8], output: &mut BitStream )
  {
    if is_lit 
    { 
      self.previous_length = 0; 
      self.zero_run = 0; 
      self.repeat = 0; 
    }
    for len in &lengths[..count]
    {
      let length = *len as usize;
      if length == 0
      { 
        if self.repeat > 0 { self.encode_repeat( output ); } 
        self.zero_run += 1; 
        self.previous_length = 0; 
      } else if length == self.previous_length {
        self.repeat += 1;
      } else { 
        if self.zero_run > 0 { self.encode_zero_run( output ); } 
        if self.repeat > 0 { self.encode_repeat( output ); }
        self.put_length( length, output );
        self.previous_length = length; 
      }
    }      
    if !is_lit 
    { 
      self.encode_zero_run( output ); 
      self.encode_repeat( output );
    }
  }

  fn put_length( &mut self, code: usize, output: &mut BitStream ) 
  { 
    if self.length_pass == 1 
    {
      self.bc.used[ code ] += 1; 
    } else {
      output.write( self.bc.bits[ code ], self.bc.code[ code ] as u64 ); 
    }
  }

  fn encode_repeat( &mut self, output: &mut BitStream )
  {
    while self.repeat > 0
    {
      if self.repeat < 3 
      { 
        self.put_length( self.previous_length, output ); 
        self.repeat -= 1; 
      } else { 
        let mut x = self.repeat; 
        if x > 6 { x = 6; } 
        self.put_length( 16, output ); 
        if self.length_pass == 2
        { 
          output.write( 2, ( x - 3 ) as u64 ); 
        }
        self.repeat -= x;  
      }
    }
  }

  fn encode_zero_run( &mut self, output: &mut BitStream )
  {
    while self.zero_run > 0
    {
      if self.zero_run < 3 
      { 
        self.put_length( 0, output ); 
        self.zero_run -= 1; 
      }
      else if self.zero_run < 11 
      { 
        self.put_length( 17, output ); 
        if self.length_pass == 2 { output.write( 3, ( self.zero_run - 3 ) as u64 ); }
        self.zero_run = 0;  
      } else { 
        let mut x = self.zero_run; 
        if x > 138 { x = 138; } 
        self.put_length( 18, output ); 
        if self.length_pass == 2 { output.write( 7, ( x - 11 ) as u64 ); } 
        self.zero_run -= x; 
      }
    }
  }

} // end impl LenCoder

/// Output bit stream.
pub struct BitStream 
{
  buffer: u64,
  bits_in_buffer : u8,
  pub bytes: Vec<u8>,
}

impl BitStream
{
  pub fn new() -> BitStream
  {
    BitStream
    {
      buffer: 0,
      bits_in_buffer: 0,
      bytes: Vec::new()
    }
  }

  /// Write first n bits of value to BitStream, least significant bit is written first.
  /// Unused bits of value must be zero, i.e. value must be in range 0 .. 2^n-1.

  pub fn write( &mut self, mut n: u8, mut value: u64 )
  {
    if n + self.bits_in_buffer >= 64
    {
      self.save( value << self.bits_in_buffer | self.buffer );
      let space = 64 - self.bits_in_buffer;
      value >>= space;
      n -= space;
      self.buffer = 0;
      self.bits_in_buffer = 0;
    }
    self.buffer |= value << self.bits_in_buffer;
    self.bits_in_buffer += n;
  }

  /// Pad output with zero bits to n bit boundary where n is power of 2 in range 1,2,4..64, typically n=8.
  pub fn pad( &mut self, n: u8 )
  {
    let w = self.bits_in_buffer % n; 
    if w > 0 { self.write( n - w, 0 ); }
  }
  
  /// Flush bit buffer to bytes.
  pub fn flush( &mut self )
  {
    self.pad( 8 );
    let mut w = self.buffer;
    while self.bits_in_buffer > 0
    {
      self.bytes.push( ( w & 255 ) as u8 ); 
      w >>= 8;
      self.bits_in_buffer -= 8;
    }
  }

  fn save( &mut self, mut w: u64 )
  {
    let b = &mut self.bytes;
    b.push( ( w & 255 ) as u8 ); w >>= 8;
    b.push( ( w & 255 ) as u8 ); w >>= 8;
    b.push( ( w & 255 ) as u8 ); w >>= 8;
    b.push( ( w & 255 ) as u8 ); w >>= 8;

    b.push( ( w & 255 ) as u8 ); w >>= 8;
    b.push( ( w & 255 ) as u8 ); w >>= 8;
    b.push( ( w & 255 ) as u8 ); w >>= 8;
    b.push( ( w & 255 ) as u8 );
  }
} // end impl BitStream

/// Reverse a string of bits ( ready to be output as Huffman code ).
pub fn reverse( mut x:u16, mut bits: usize ) -> u16
{ 
  let mut result: u16 = 0; 
  while bits > 0
  {
    result = ( result << 1 ) | ( x & 1 ); 
    x >>= 1; 
    bits -= 1;
  } 
  result
} 
