use crate::matcher;
use crate::matcher::Match;
use crate::bit::BitStream;
use crate::block::Block;

use scoped_threadpool::Pool;
use crossbeam::{channel,Receiver};

pub fn compress( inp: &[u8], p: &mut Pool ) -> Vec<u8>
{
  let mut out = BitStream::new();
  let ( tx, rx ) = channel::unbounded();

  // Execute the match finding and block output in parallel using the scoped thread pool.
  p.scoped( |s| 
  {
    s.execute( || { matcher::find( inp, tx ); } );
    do_blocks( inp, rx, &mut out );
  } );

  out.bytes
}

pub fn do_blocks( inp: &[u8], rx: Receiver<Match>, out: &mut BitStream )
{
  out.write( 16, 0x9c78 );

  let len = inp.len();
  let mut ii = 0; // input index
  let mut mi = 0; // match index
  let mut mp = 0; // match position
  let mut mlist : Vec<Match> = Vec::new();
  loop
  {
    let mut block_size = len - ii;
    if block_size > 0x4000 { block_size = 0x4000; }
    let mut b = Block::new( ii, block_size, mi );

    while mp < b.input_end // Get matches for the block.
    {
      match rx.recv()
      {
        Ok( m ) => 
        {
          mp = m.position;
          mlist.push( m );          
        },
        Err( _err ) => mp = len
      }
    }

    b.init( &inp, &mlist );
    ii = b.input_end;
    mi = b.match_end;
    b.write( &inp, &mlist, out, ii == len );
    if ii == len { break; }
  }   
  out.pad(8);
  out.write( 32, adler32( &inp ) as u64 );
  out.flush();

  // println!( "Total matches={}", mlist.len() );
}

/*
pub fn compress( inp: &[u8] ) -> Vec<u8>
{
  let mut out = BitStream::new();
  out.write( 16, 0x9c78 );

  let mut mlist : Vec<Match> = Vec::new();
  matcher::find( inp, &mut mlist );

  // for mat in &mlist { println!( "Match at {} length={} distance={}", mat.position, mat.length, mat.distance ); } }
  // println!( "Total matches={}", mlist.len() );

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
*/

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
