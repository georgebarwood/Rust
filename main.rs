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

 let n = 100;
 compress::test( n );
 println!( "test completed ok, n={} time elapsed={} ms.", n, start.elapsed().as_millis() );
}
