// use rand::Rng;
use std::time::Instant;

mod compress;
mod bit;
mod col;
mod matcher;
mod block;
mod inflate;

fn main() 
{
 let start = Instant::now();

 let n = 10;
 test( n );
 println!( "test completed ok, n={} time elapsed={} micro sec.", n, start.elapsed().as_micros() );
}

pub fn test( n:usize )
{
  // let f = std::fs::read( "C:\\PdfFiles\\FreeSans.ttf" ).unwrap();

  for _i in 0..n
  {
    check( &[1,2,3,4], &[120,156,5,128,1,9,0,0,0,130,40,253,191,89,118,12,11,0,24,0] );
    check( &[0,0,0,0,1,2,3,4], &[120,156,13,192,5,1,0,0,0,194,48,172,127,102,62,193,233,14,11,0,28,0] );
    check( &[1,2,3,4,1,2,3,4,1,2,3,4,1,1,4,1,2,3,4], &[] );
    let mut t : Vec<u8> = Vec::new();
    for i in 0..10000 { t.push( ( ( i % 256 ) | ( i % 13 ) ) as u8 ); }
    check( &t, &[] );
    // check( &f, &[] );
  }
}

pub fn check( inp: &[u8], chk: &[u8] )
{
  let cb : &[u8] = &compress::compress( inp );

  for i in 0..chk.len()
  {
    // println!( "i={} b={}", i, cb[i] );
    // if chk[i] != cb[i] { println!( "Failed at i={}", i ); }
    assert_eq!( chk[i], cb[i] );
  }
  //println!( "test ran ok inp.len={} cb.len={}", inp.len(), cb.len() );

  let inf = inflate::inflate( cb );
  for i in 0..inp.len()
  {
    assert_eq!( inf[i], inp[i] );
  }
}
