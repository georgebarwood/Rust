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
    if block_size > 0x4000 { block_size = 0x4000; }
    let mut b = Block::new( ii, block_size, mi );
    b.init( &inp, &mlist );
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

