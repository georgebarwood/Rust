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
 let n = 100;

 {
   let start = Instant::now();
   flate_test(n);
   println!( "flate2 test completed ok, n={} time elapsed={} milli sec.", n, start.elapsed().as_millis() );
 }

 {
   let mut config = compress::Config::new();
   // config.options.dynamic_block_size = true;
   // config.options.block_size = 0x1000;
   // config.options.probe_max = 5;

   // config.options.lazy_match = false;


   let data = [ 1,2,3,4,1,2,3 ];
   let cb : Vec<u8> = compress::compress( &data, &mut config );
   let _ub : Vec<u8> = inflate::inflate( &cb );

   let start = Instant::now();
   test( n, &mut config );
   println!( "flate3 test completed ok, n={} time elapsed={} milli sec.", n, start.elapsed().as_millis() );
 }
}

pub fn test( n:usize, c: &mut compress::Config )
{
/*
    check( &[1,2,3,4], &[120,156,5,128,1,9,0,0,0,130,40,253,191,89,118,12,11,0,24,0], c );
    check( &[0,0,0,0,1,2,3,4], &[120,156,13,192,5,1,0,0,0,194,48,172,127,102,62,193,233,14,11,0,28,0], c );
    check( &[1,2,3,4,1,2,3,4,1,2,3,4,1,1,4,1,2,3,4], &[], c );
    let mut t : Vec<u8> = Vec::new();
 
    let mut rng = rand::thread_rng();
    for _i in 0..10000 
    { 
      // t.push( ( ( i % 256 ) | ( i % 13 ) ) as u8 ); 
      // t.push( ( i % 13 ) as u8 );
      t.push( rng.gen() ); 
    }
    check( &t, &[], c );
*/
  let f = std::fs::read( "C:\\PdfFiles\\FreeSans.ttf" ).unwrap();
  check( n, &f, &[], c );
}

pub fn check( n:usize, inp: &[u8], chk: &[u8], c: &mut compress::Config )
{
  let mut csize = 0;

  for _loop in 0..n
  {
    let cb = compress::compress( inp, c );
    csize = cb.len();

    for i in 0..chk.len()
    {
      // println!( "i={} b={}", i, cb[i] );
      // if chk[i] != cb[i] { println!( "Failed at i={}", i ); }
      assert_eq!( chk[i], cb[i] );
    }

/*
    let inf = inflate::inflate( &cb );
    for i in 0..inp.len()
    {
      assert_eq!( inf[i], inp[i] );
    }
*/
  }

  println!( "flate3 compressed size={}", csize );
}

fn flate_test( n:usize )
{
  let f = std::fs::read( "C:\\PdfFiles\\FreeSans.ttf" ).unwrap();

  use flate2::write::GzEncoder;
  use flate2::Compression;
  use std::io::prelude::*;

  let mut csize = 0;

  for _i in 0..n
  {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default() /*new(9)*/ );
    encoder.write_all(&f).unwrap();
    let compressed_bytes = encoder.finish().unwrap();
    csize = compressed_bytes.len();
  }
  println!( "flate2 compressed size={}", csize );
 
}

