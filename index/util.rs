/// Extract unsigned value of n bytes from data[off].
pub fn get( data: &[u8], off: usize, n: usize ) -> u64
{
  let mut x = 0;
  for i in 0..n
  {
    x = ( x << 8 ) + data[ off + n - i - 1 ] as u64;
  }
  x
}

/// Store unsigned value of n bytes to data[off].
pub fn set( data: &mut[u8], off: usize, mut val:u64, n: usize )
{
  for i in 0..n
  {
    data[ off + i ] = ( val & 255 ) as u8;
    val >>= 8;
  }
}

// Bitfield  macros

// The mask to extract $len bits at bit offset $off
#[macro_export]
macro_rules! mask
{
  ($off: expr, $len: expr ) => 
  { ( ( 1 << $len ) - 1 ) << $off }
}

// Extract $len bits from $val at bit offset $off
#[macro_export]
macro_rules! get
{
  ( $val: expr, $off: expr, $len: expr ) =>
  { ( $val & mask!($off,$len) ) >> $off }
}

// Update $len bits in $var at bit offset $off to $val
#[macro_export]
macro_rules! set
{
  ( $var: expr, $off: expr, $len: expr, $val: expr ) =>
  { $var = ( $var & ! mask!($off,$len) ) 
     | ( ( $val << $off ) & mask!($off,$len) )
  }
}