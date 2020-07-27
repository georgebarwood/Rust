use scoped_threadpool::Pool;
use crossbeam::{channel,Receiver};

use crate::matcher;
use crate::matcher::Match;
use crate::bit::BitStream;
use crate::block::Block;

pub fn compress( inp: &[u8], p: &mut Pool ) -> Vec<u8>
{
  let mut out = BitStream::new();
  let ( mtx, mrx ) = channel::bounded(1000); // channel for matches
  let ( ctx, crx ) = channel::bounded(1); // channel for checksum

  // Execute the match finding, checksum computation and block output in parallel using the scoped thread pool.
  p.scoped( |s| 
  {
    s.execute( || { matcher::find( inp, mtx ); } );
    s.execute( || { ctx.send( adler32( &inp ) ).unwrap(); } );
    write_blocks( inp, mrx, crx, &mut out );
  } );

  out.bytes
}

pub fn write_blocks( inp: &[u8], mrx: Receiver<Match>, crx: Receiver<u32>, out: &mut BitStream )
{
  out.write( 16, 0x9c78 );

  let len = inp.len();
  let mut block_start = 0; // start of next block
  let mut match_start = 0; // start of matches for next block
  let mut match_position = 0; // latest match position
  let mut mlist : Vec<Match> = Vec::new(); // list of matches
  loop
  {
    let mut block_size = len - block_start;
    if block_size > 0x4000 { block_size = 0x4000; }
    let mut b = Block::new( block_start, block_size, match_start );

    while match_position < b.input_end // Get matches for the block.
    {
      match mrx.recv()
      {
        Ok( m ) => 
        {
          match_position = m.position;
          mlist.push( m );          
        },
        Err( _err ) => match_position = len
      }
    }

    b.init( &inp, &mlist );
    block_start = b.input_end;
    match_start = b.match_end;
    b.write( &inp, &mlist, out, block_start == len );
    if block_start == len { break; }
  }   
  out.pad(8);
  out.write( 32, crx.recv().unwrap() as u64 );
  out.flush();
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
