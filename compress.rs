use crossbeam::{channel,Receiver};

use crate::matcher;
use crate::matcher::Match;
use crate::bit::BitStream;
use crate::block::Block;

pub struct Options
{
  pub dynamic_block_size: bool,
  pub block_size: usize,
  pub probe_max: usize, 
  pub lazy_match: bool
}

pub struct Config
{
  pub options: Options,
  pub pool: scoped_threadpool::Pool
}

impl Config
{
  pub fn new() -> Config
  {
    Config
    { 
      options: Options
      { 
        dynamic_block_size: false, 
        block_size: 0x2000, 
        probe_max: 10, 
        lazy_match: true 
      },
      pool: scoped_threadpool::Pool::new(2)
    }
  }
}

/// Example:
/// let config = compress::Config::new();
/// let data = [ 1,2,3,4,1,2,3 ];
/// let cb : Vec<u8> = compress::compress( &data, &mut c );
/// println!( "compressed size={}", cb.len() );

pub fn compress( inp: &[u8], c: &mut Config ) -> Vec<u8>
{
  let mut out = BitStream::new( inp.len() );
  let ( mtx, mrx ) = channel::bounded(1000); // channel for matches
  let ( ctx, crx ) = channel::bounded(1); // channel for checksum

  let opts = &c.options;

  // Execute the match finding, checksum computation and block output in parallel using the scoped thread pool.
  c.pool.scoped( |s| 
  {
    s.execute( || { matcher::find( inp, mtx , &opts ); } );
    s.execute( || { ctx.send( adler32( &inp ) ).unwrap(); } );
    write_blocks( inp, mrx, crx, &mut out, &opts );
  } );

  out.bytes
}

pub fn write_blocks( inp: &[u8], mrx: Receiver<Match>, crx: Receiver<u32>, out: &mut BitStream, opt: &Options )
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
    let mut target_size = opt.block_size;
    if block_size > target_size { block_size = target_size; }

    let mut b = Block::new( block_start, block_size, match_start );
    match_position = get_matches( match_position, b.input_end, &mrx, &mut mlist );
    b.init( &inp, &mlist );

    if opt.dynamic_block_size // Investigate larger block size.
    {
      let mut bits = b.bit_size( out );
      loop
      {
        // b2 is a block which starts just after b, same size.
        block_size = len - b.input_end;
        if block_size == 0 { break; }
        target_size = b.input_end - b.input_start;
        if block_size > target_size { block_size = target_size; }
        let mut b2 = Block::new( b.input_end, block_size, b.match_end );
        match_position = get_matches( match_position, b2.input_end, &mrx, &mut mlist );
        b2.init( &inp, &mlist );

        // b3 covers b and b2 exactly as one block.
        let mut b3 = Block::new( b.input_start, b2.input_end - b.input_start, b.match_start );
        b3.init( &inp, &mlist );

        let bits2 = b2.bit_size( out );
        let bits3 = b3.bit_size( out ); 

        if bits3 > bits + bits2 
        {
          // tune_boundary( b, b2 ); 
          break; 
        }
        b = b3;
        bits = bits3;
      }
    }

    block_start = b.input_end;
    match_start = b.match_end;

    // println!( "block size={} start={} end={}", b.input_end - b.input_start, b.input_start, b.input_end );

    b.write( &inp, &mlist, out, block_start == len );
    if b.input_end == len { break; }
  }   
  out.pad(8);
  out.write( 32, crx.recv().unwrap() as u64 );
  out.flush();
}

/// Get matches up to position.
fn get_matches( mut match_position: usize, to_position: usize, mrx: &Receiver<Match>, mlist: &mut Vec<Match> ) -> usize
{
  while match_position < to_position 
  {
    match mrx.recv()
    {
      Ok( m ) => 
      {
        match_position = m.position;
        mlist.push( m );          
      },
      Err( _err ) => match_position = usize::MAX
    }
  }
  match_position
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
