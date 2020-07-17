use crate::matcher;
use crate::matcher::Match;
use crate::bit::BitStream;
use crate::block::Block;

pub fn compress( inp: &[u8] ) -> Vec<u8>
{
  let mut out = BitStream::new();
  out.write( 16, 0x9c78 );

  let mut mlist : Vec<Match> = Vec::new();
  matcher::find( inp, &mut mlist );

  // for mat in &mlist { println!( "Match at {} length={} distance={}", mat.position, mat.length, mat.distance ); } }

  let len = inp.len();
  let mut ii = 0; // input index
  let mut mi = 0; // match index
  loop
  {
    let mut block_size = len - ii;
    if block_size > 0x1000 { block_size = 0x1000; }
    let mut b = Block::new( ii, block_size, mi );
    b.init( &inp, &mlist );
    // println!( "block size={}", b.input_end - ii ); 
    ii = b.input_end;
    mi = b.match_end;
    b.write( &inp, &mlist, &mut out, ii == len );
    if ii == len { break; }
  }   
  out.pad(8);
  out.write( 32, adler32( &inp ) as u64 );
  out.flush();
  out.bytes
}

pub fn test( n:usize )
{
  check( &[1,2,3,4], &[120,156,5,128,1,9,0,0,0,130,40,253,191,89,118,12,11,0,24,0] );
  check( &[0,0,0,0,1,2,3,4], &[120,156,13,192,5,1,0,0,0,194,48,172,127,102,62,193,233,14,11,0,28,0] );
  check( &[1,2,3,4,1,2,3,4,1,2,3,4,1,1,4,1,2,3,4], &[] );
  let mut t : Vec<u8> = Vec::new();
  for i in 0..n { t.push( ( ( i % 256 ) | ( i % 13 ) ) as u8 ); }
  check( &t, &[] );
}

pub fn check( inp: &[u8], chk: &[u8] )
{
  let cb : &[u8] = &compress( inp );

  for i in 0..chk.len()
  {
    // println!( "i={} b={}", i, cb[i] );
    if chk[i] != cb[i] { println!( "Failed at i={}", i ); }
    assert_eq!( chk[i], cb[i] );
  }
  // println!( "test ran ok inp.len={} cb.len={}", inp.len(), cb.len() );
}

/// Checksum function per RFC 1950.
pub fn adler32( input: &[u8] ) -> u32
{
  let mut s1 = 1;
  let mut s2 = 0;
  for b in input
  {
    s1 = ( s1 + *b as u32 ) % 65521;
    s2 = ( s2 + s1 ) % 65521;
  }
  s2 * 65536 + s1   
}
