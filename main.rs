// use rand::Rng;
use std::time::Instant;
use scoped_threadpool::Pool;

mod compress;
mod bit;
mod col;
mod matcher;
mod block;
mod inflate;

fn main() 
{
 let n = 100;

 let mut pool = Pool::new(4); 

 //let data = [ 1,2,3,4,1,2,3 ];
 //let cb : Vec<u8> = compress::compress( &data, &mut pool );
 //println!( "compressed size={}", cb.len() );

 let start = Instant::now();
 test( n, &mut pool );
 // flate_test(n);
 println!( "test completed ok, n={} time elapsed={} milli sec.", n, start.elapsed().as_millis() );
}

pub fn test( n:usize, p: &mut Pool )
{
  let f = std::fs::read( "C:\\PdfFiles\\FreeSans.ttf" ).unwrap();

  for _i in 0..n
  {
/*
    check( &[1,2,3,4], &[120,156,5,128,1,9,0,0,0,130,40,253,191,89,118,12,11,0,24,0], p );
    check( &[0,0,0,0,1,2,3,4], &[120,156,13,192,5,1,0,0,0,194,48,172,127,102,62,193,233,14,11,0,28,0], p );
    check( &[1,2,3,4,1,2,3,4,1,2,3,4,1,1,4,1,2,3,4], &[], p );
    let mut t : Vec<u8> = Vec::new();
 
    let mut rng = rand::thread_rng();
    for _i in 0..10000 
    { 
      // t.push( ( ( i % 256 ) | ( i % 13 ) ) as u8 ); 
      // t.push( ( i % 13 ) as u8 );
      t.push( rng.gen() ); 
    }
    check( &t, &[], p );
*/

    check( &f, &[], p );
  }
}

pub fn check( inp: &[u8], chk: &[u8], p: &mut Pool )
{
  for _loop in 0..1
  {
    let cb = compress::compress( inp, p );

    // println!("cb len={}", cb.len() );

    for i in 0..chk.len()
    {
      // println!( "i={} b={}", i, cb[i] );
      // if chk[i] != cb[i] { println!( "Failed at i={}", i ); }
      assert_eq!( chk[i], cb[i] );
    }

    let inf = inflate::inflate( &cb );
    for i in 0..inp.len()
    {
      assert_eq!( inf[i], inp[i] );
    }
  }

/*
  let mut deflater = DeflateDecoder::new( cb );
  let mut x = Vec::<u8>::new();
  deflater.read_to_end(&mut x).unwrap();
  println!( "x.len={}", x.len() );
*/

/*
  for i in 0..inp.len()
  {
    assert_eq!( x[i], inp[i] );
  }
*/
  //let mut s = String::new();
  //deflater.read_to_string(&mut s);


}

/*
fn flate_test( n:usize )
{
  let f = std::fs::read( "C:\\PdfFiles\\FreeSans.ttf" ).unwrap();

  use flate2::write::GzEncoder;
  use flate2::Compression;
  use std::io::prelude::*;

  for _i in 0..n
  {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::new(9) );
    encoder.write_all(&f).unwrap();
    let compressed_bytes = encoder.finish().unwrap();
    // println!( "flate compressed size={}", compressed_bytes.len() );
  }
}
*/
