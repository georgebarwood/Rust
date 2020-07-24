// use std::sync::mpsc::Sender;
use crossbeam::channel::Sender;

pub struct Match
{
  pub position: usize,
  pub length: usize,
  pub distance: usize
}

pub fn find( input: &[u8], output: &mut Vec<Match> )
{
  let len = input.len();
  if len > MIN_MATCH
  {
    let mut m = Matcher::new( len );
    m.find( input, output );
  }
}

pub fn find_par( input: &[u8], output: Sender<Match> )
{
  let len = input.len();
  if len > MIN_MATCH
  {
    let mut m = Matcher::new( len );
    m.find_par( input, output );
  }
}

// RFC 1951 match ( LZ77 ) limits.
const MIN_MATCH : usize = 3; // The smallest match eligible for LZ77 encoding.
const MAX_MATCH : usize = 258; // The largest match eligible for LZ77 encoding.
const MAX_DISTANCE : usize = 0x8000; // The largest distance backwards in input from current position that can be encoded.
const ENCODE_POSITION : usize = MAX_DISTANCE + 1;

struct Matcher
{
  hash_shift: usize,
  hash_mask: usize,
  hash_table: Vec<usize>
}

impl Matcher
{
  fn new( len: usize ) -> Matcher
  {
    let hash_shift = calc_hash_shift( len * 2 );
    let hash_mask = ( 1 << ( MIN_MATCH * hash_shift ) ) - 1;

    Matcher{
      hash_shift,
      hash_mask,
      hash_table: vec![ 0; hash_mask + 1 ]
    } 
  }

  fn find( &mut self, input: &[u8], output: &mut Vec<Match> ) // LZ77 compression.
  {
    let limit = input.len() - 2;

    let mut link : Vec<usize> = vec!(0; limit);

    let mut position = 0; // position in input.

    // hash will be hash of three bytes starting at position.
    let mut hash = ( ( input[ 0 ] as usize ) << self.hash_shift ) + input[ 1 ] as usize;

    while position < limit
    {
      hash = ( ( hash << self.hash_shift ) + input[ position + 2 ] as usize ) & self.hash_mask;        
      let mut hash_entry = self.hash_table[ hash ];
      self.hash_table[ hash ] = position + ENCODE_POSITION;

      if position >= hash_entry // Equivalent to position - ( hash_entry - ENCODE_POSITION ) > MAX_DISTANCE.
      {
         position += 1;
         continue;
      }
      link[ position ] = hash_entry;

      let ( mut match1, mut distance1 ) = self.best_match( input, position, hash_entry - ENCODE_POSITION, &mut link );
      position += 1;
      if match1 < MIN_MATCH { continue; }

      // "Lazy matching" RFC 1951 p.15 : if there are overlapping matches, there is a choice over which of the match to use.
      // Example: "abc012bc345.... abc345". Here abc345 can be encoded as either [abc][345] or as a[bc345].
      // Since a range typically needs more bits to encode than a single literal, choose the latter.
      while position < limit
      {
        hash = ( ( hash << self.hash_shift ) + input[ position + 2 ] as usize ) & self.hash_mask;          
        hash_entry = self.hash_table[ hash ];

        self.hash_table[ hash ] = position + ENCODE_POSITION;
        if position >= hash_entry { break; }
        link[ position ] = hash_entry;

        let ( match2, distance2 ) = self.best_match( input, position, hash_entry - ENCODE_POSITION, &mut link );
        if match2 > match1 || match2 == match1 && distance2 < distance1
        {
          match1 = match2;
          distance1 = distance2;
          position += 1;
        }
        else { break; }
      }

      // println!( "Found match at {} length={} distance={}", position-1, match1, distance1 );
      output.push( Match{ position:position-1, length:match1, distance:distance1 } );

      let mut copy_end = position - 1 + match1;
      if copy_end > limit { copy_end = limit; }

      position += 1;

      // Advance to end of copied section.
      while position < copy_end
      { 
        hash = ( ( hash << self.hash_shift ) + input[ position + 2 ] as usize ) & self.hash_mask;
        link[ position ] = self.hash_table[ hash ];
        self.hash_table[ hash ] = position + ENCODE_POSITION;
        position += 1;
      }
    }
  }

  fn find_par( &mut self, input: &[u8], output: Sender<Match> ) // LZ77 compression.
  {
    let limit = input.len() - 2;

    let mut link : Vec<usize> = vec!(0; limit);

    let mut position = 0; // position in input.

    // hash will be hash of three bytes starting at position.
    let mut hash = ( ( input[ 0 ] as usize ) << self.hash_shift ) + input[ 1 ] as usize;

    while position < limit
    {
      hash = ( ( hash << self.hash_shift ) + input[ position + 2 ] as usize ) & self.hash_mask;        
      let mut hash_entry = self.hash_table[ hash ];
      self.hash_table[ hash ] = position + ENCODE_POSITION;

      if position >= hash_entry // Equivalent to position - ( hash_entry - ENCODE_POSITION ) > MAX_DISTANCE.
      {
         position += 1;
         continue;
      }
      link[ position ] = hash_entry;

      let ( mut match1, mut distance1 ) = self.best_match( input, position, hash_entry - ENCODE_POSITION, &mut link );
      position += 1;
      if match1 < MIN_MATCH { continue; }

      // "Lazy matching" RFC 1951 p.15 : if there are overlapping matches, there is a choice over which of the match to use.
      // Example: "abc012bc345.... abc345". Here abc345 can be encoded as either [abc][345] or as a[bc345].
      // Since a range typically needs more bits to encode than a single literal, choose the latter.
      while position < limit
      {
        hash = ( ( hash << self.hash_shift ) + input[ position + 2 ] as usize ) & self.hash_mask;          
        hash_entry = self.hash_table[ hash ];

        self.hash_table[ hash ] = position + ENCODE_POSITION;
        if position >= hash_entry { break; }
        link[ position ] = hash_entry;

        let ( match2, distance2 ) = self.best_match( input, position, hash_entry - ENCODE_POSITION, &mut link );
        if match2 > match1 || match2 == match1 && distance2 < distance1
        {
          match1 = match2;
          distance1 = distance2;
          position += 1;
        }
        else { break; }
      }

      // println!( "Found match at {} length={} distance={}", position-1, match1, distance1 );
      output.send( Match{ position:position-1, length:match1, distance:distance1 } ).unwrap();

      let mut copy_end = position - 1 + match1;
      if copy_end > limit { copy_end = limit; }

      position += 1;

      // Advance to end of copied section.
      while position < copy_end
      { 
        hash = ( ( hash << self.hash_shift ) + input[ position + 2 ] as usize ) & self.hash_mask;
        link[ position ] = self.hash_table[ hash ];
        self.hash_table[ hash ] = position + ENCODE_POSITION;
        position += 1;
      }
    }
  }

  // best_match finds the best match starting at position. 
  // old_position is from hash table, link [] is linked list of older positions.

  fn best_match( &mut self, input: &[u8], position: usize, mut old_position: usize, link: &mut Vec<usize> ) -> ( usize, usize )
  { 
    let mut avail = input.len() - position;
    if avail > MAX_MATCH { avail = MAX_MATCH; }

    let mut best_match = 0; let mut best_distance = 0;
    let mut key_byte = input[ position + best_match ];

    loop
    { 
      if input[ old_position + best_match ] == key_byte
      {
        let mut mat = 0; 
        while mat < avail && input[ position + mat ] == input[ old_position + mat ]
        {
          mat += 1;
        }
        if mat > best_match
        {
          best_match = mat;
          best_distance = position - old_position;
          if best_match == avail || ! self.match_possible( input, position, best_match ) { break; }
          key_byte = input[ position + best_match ];
        }
      }
      old_position = link[ old_position ];
      if old_position <= position { break; }
      old_position -= ENCODE_POSITION;
    }
    ( best_match, best_distance )
  }

  // match_possible is used to try and shorten the best_match search by checking whether 
  // there is a hash entry for the last 3 bytes of the next longest possible match.

  fn match_possible( &mut self, input: &[u8], mut position: usize, best_match: usize ) -> bool
  {
    position = ( position + best_match ) - 2;
    let mut hash = ( ( input[ position ] as usize ) << self.hash_shift ) + input[ position + 1 ] as usize;
    hash = ( ( hash << self.hash_shift ) + input[ position + 2 ] as usize ) & self.hash_mask;        
    position < self.hash_table[ hash ]
  }
} // end impl Matcher

fn calc_hash_shift( n: usize ) -> usize
{
  let mut p = 1;
  let mut result = 0;
  while n > p
  {
    p <<= MIN_MATCH;
    result += 1;
    if result == 6 { break; }
  }
  result
} 
