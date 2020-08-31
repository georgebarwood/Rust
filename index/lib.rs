/// Utility functions.
#[macro_use] pub mod util;

/// Sorted Record storage.
pub mod index;

/// SQL ( Structured Query Language ).
pub mod sql; 

/// A record to be stored in a file.
pub trait Record
{
  fn save( &self, data:&mut [u8], off: usize, both: bool );
  fn load( &mut self, data: &[u8], off: usize, both: bool );
  fn compare( &self, data: &[u8], off: usize ) -> std::cmp::Ordering;
  fn key( &self, data:&[u8], off: usize ) -> Box<dyn Record>;
}

/// Backing storage for a file.
pub trait BackingStorage
{
  fn size( &mut self ) -> u64;
  fn read( &mut self, off: u64, data: &mut[u8] );
  fn save( &mut self, off: u64, data: &[u8] );
}
