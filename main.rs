// use rand::Rng;
mod compress;
mod bit;
mod col;
mod matcher;
mod block;
mod inflate;

fn main() 
{
  // inflate::test();

  let n = 1000; // 1000;
  let m = 10000;
  for _i in 0..n { compress::test( m ); }
  println!( "{} x {} test completed ok.", n, m );
  
}
