/// Sorted Record storage.
pub struct File<'stg>
{
  pub pages: Vec<Page>,
  pub rec_size: usize,
  pub key_size: usize,
  pub store: &'stg mut dyn BackingStorage
}

/// Retrieve records from a File.
pub struct Cursor<'stg,'file,'start>
{
  stk: Stack<'start>,
  ixf: &'file mut File<'stg>
}

/// The size in bytes of each page.
pub const PAGE_SIZE : usize = 0x4000;

#[derive(Default)]
/// A page in a File.
pub struct Page
{
  pub data: Vec<u8>, // Data storage.
  node_size: usize,  // Number of bytes required for each node.
  root: usize,       // Root node.
  pub count: usize,  // Number of Records currently stored.
  free: usize,       // First Free node.
  alloc: usize, // Number of Nodes currently allocated.

  first_page: usize, // First child page ( for a non-leaf page ).
  pub parent: bool,  // Is page a parent page?
  pub dirty: bool,   // Does page need to be saved to backing storage?
}

use std::cmp::Ordering;
use crate::util::{get,set};
use crate::Record;
use crate::BackingStorage;


impl <'stg> File<'stg>
{

  /// Create File with specified record size, key size and BackingStorage.
  pub fn new( rec_size: usize, key_size: usize, store: &'stg mut dyn BackingStorage ) -> File<'stg>
  {
    let page_count = ( ( store.size() + PAGE_SIZE as u64 - 1 ) / PAGE_SIZE as u64 ) as usize;
    let mut result = File
    { 
      pages: Vec::with_capacity( page_count ), 
      rec_size, 
      key_size,
      store 
    };

    if page_count == 0
    {
      result.pages.push( result.new_page( false ) );
    } else {
      for _i in 0..page_count
      {
        result.pages.push( Page::default() );
      }
    }
    result    
  }

  /// Insert a Record.
  pub fn insert( &mut self, r: &dyn Record )
  {
    self.insert_leaf( 0, r, None );
  }

  /// Remove a Record.
  pub fn remove( &mut self, r: &dyn Record )
  {
    let mut p = self.load_page( 0 );
    while p.parent
    {
      let x = p.find_node( r );
      let cp = if x == 0 { p.first_page } else { p.child( x ) };
      p = self.load_page( cp );
    }
    p.remove( r );
  }

  /// Obtain a cursor to iterate over the Records.
  pub fn cursor <'file,'start>( &'file mut self, start: &'start dyn Record ) -> Cursor<'stg,'file,'start>
  {
    Cursor::new( start, self )
  }

  /// Save the changed pages to BackingStorage.
  pub fn save( &mut self, free_mem:bool )
  {
    let n = self.pages.len();
    for i in 0..n
    {
      let p = &mut self.pages[i];
      if p.dirty
      {
        p.write_header();
        if i == n - 1
        {
          self.store.save( ( i as u64 ) * (PAGE_SIZE as u64), &p.data[0..p.size()] );
        } else {
          self.store.save( ( i as u64 ) * ( PAGE_SIZE as u64), &p.data );
        }
        p.dirty = false;
      }
      if free_mem && i > 0
      {
        self.pages[i] = Page::default();
      }
    }
  }

  fn insert_leaf( &mut self, pnum: usize, r: &dyn Record, pi: Option<&ParentInfo> )
  {
    let p = self.load_page( pnum );
    if p.parent
    {
      // Look for child page to insert into.
      let x = p.find_node( r );
      let cp = if x == 0 { p.first_page } else { p.child( x ) };
      self.insert_leaf( cp, r, Some(&ParentInfo{ pnum, parent:pi }) );
    } else if !p.full() {
      p.insert( r );
    }  else {
      // Page is full, divide it into left and right.
      let sp = Split::new( p );
      let sk = &*p.get_key( sp.split_node, r );

      // Could insert r into left or right here.

      let pnum2 = self.pages.len();
      self.pages.push( sp.right );
      match pi 
      {
        None =>
        {
          // New root page needed.
          let mut new_root = self.new_page( true );
          new_root.first_page = self.pages.len();
          self.pages.push( sp.left );
          self.pages[ 0 ] = new_root;
          self.append_page( 0, sk, pnum2 );
        },
        Some( pi ) =>
        {  
          self.pages[ pnum ] = sp.left;
          self.insert_page( pi, sk, pnum2 );
        }
      }
      self.insert( r ); // Could be avoided by inserting into left or right above.
    }
  } 

  fn insert_page( &mut self, into: &ParentInfo, r:&dyn Record, cpnum: usize )
  {
    let p = &mut self.pages[ into.pnum ];
    // Need to check if page is full.
    if !p.full() 
    {
      p.insert_child( r, cpnum );
    } else {
      // Split the parent page.

      let mut sp = Split::new( p );
      let sk = &*p.get_key( sp.split_node, r );

      // Insert into either left or right.
      let c = p.compare( r, sp.split_node );
      if c == Ordering::Greater 
      { 
        sp.left.insert_child( r, cpnum ) 
      } else { 
        sp.right.insert_child( r, cpnum ) 
      }

      let pnum2 = self.pages.len();
      self.pages.push( sp.right );
     
      match into.parent
      {
        None =>
        {
          // New root page needed.
          let mut new_root = self.new_page( true );
          new_root.first_page = self.pages.len();
          self.pages.push( sp.left );
          self.pages[ 0 ] = new_root;
          self.append_page( 0, sk, pnum2 );
        },
        Some( pi ) =>
        {  
          self.pages[ into.pnum ] = sp.left;
          self.insert_page( pi, sk, pnum2 );
        }
      }
    }   
  }

  fn append_page( &mut self, into: usize, k:&dyn Record, pnum: usize )
  {
    let p = &mut self.pages[ into ];
    p.append_child( k, pnum );
  }

  fn new_page( &self, parent:bool ) -> Page
  {
    Page::new( if parent {self.key_size} else {self.rec_size}, parent, vec![0;PAGE_SIZE] )
  }

  fn load_page( &mut self, pnum: usize ) -> &mut Page
  {
    if self.pages[ pnum ].data.is_empty()
    {
      let mut data = vec![ 0; PAGE_SIZE ];
      self.store.read( ( pnum as u64 ) * ( PAGE_SIZE as u64 ), &mut data );
      let parent = data[0] & 1 != 0;
      self.pages[ pnum ] = Page::new( if parent {self.key_size} else {self.rec_size}, parent, data );
    }
    &mut self.pages[ pnum ]
  }
} // end impl File

// *********************************************************************

const NODE_OVERHEAD : usize = 3; // Size of Balance,Left,Right in a Node ( 2 + 2 x 11 = 24 bits = 3 bytes ).
const NODE_BASE : usize = 6; // 45 bits ( 1 + 4 x 11 ) needs 6 bytes.
const PAGE_ID_SIZE : usize = 6; // Number of bytes used to store a page number.

const LEFT_HIGHER : u8 = 0;
const BALANCED : u8 = 1;
const RIGHT_HIGHER : u8 = 2;

const NODE_ID_BITS : usize = 11; // Node ids are 11 bits.
const MAX_NODE : usize = bitmask!( 0, NODE_ID_BITS );

impl Page
{
  fn new( rec_size:usize, parent:bool, data: Vec<u8> ) -> Page
  {
    let node_size = NODE_OVERHEAD + rec_size + if parent {PAGE_ID_SIZE} else {0};

    let u = get( &data, 0, NODE_BASE );
    let root  = getbits!( u, 1               , NODE_ID_BITS ) as usize;
    let count = getbits!( u, 1+NODE_ID_BITS  , NODE_ID_BITS ) as usize;
    let free  = getbits!( u, 1+NODE_ID_BITS*2, NODE_ID_BITS ) as usize;
    let alloc = getbits!( u, 1+NODE_ID_BITS*3, NODE_ID_BITS ) as usize;

    let first_page = if parent { get( &data, NODE_BASE + alloc * node_size , PAGE_ID_SIZE ) } else {0} as usize;

    Page
    {
      data,
      node_size,
      root, 
      count,
      free,
      alloc,
      first_page,
      parent,
      dirty: false,
    }
  }

  fn write_header(&mut self) // Called just before page is saved to file.
  { 
    let u  = 
    if self.parent {1} else {0}
    | ( ( self.root as u64 ) << 1 )
    | ( ( self.count as u64 ) << (1+NODE_ID_BITS) )
    | ( ( self.free as u64 ) << (1+2*NODE_ID_BITS) )
    | ( ( self.alloc as u64 ) << (1+3*NODE_ID_BITS) );

    set( &mut self.data, 0, u, NODE_BASE );
    if self.parent
    { 
      let off = self.size() - PAGE_ID_SIZE;
      set( &mut self.data, off, self.first_page as u64, PAGE_ID_SIZE );
    }
  }

  pub fn size( &self ) -> usize
  {
    NODE_BASE + self.alloc * self.node_size + if self.parent {PAGE_ID_SIZE} else {0}
  }

  fn full( &self ) -> bool
  {
    self.free == 0 && ( self.alloc == MAX_NODE ||
     NODE_BASE + ( self.alloc + 1 ) * self.node_size
     + if self.parent {PAGE_ID_SIZE} else {0} >= PAGE_SIZE )
  }

  fn rec_size( &self ) -> usize
  {
    self.node_size - NODE_OVERHEAD - if self.parent { PAGE_ID_SIZE } else { 0 }
  }

  fn new_page( &self ) -> Page
  {
    Page::new( self.rec_size(), self.parent, vec![ 0; PAGE_SIZE ] )
  }

  fn split( &self, x:usize, sp:&mut Split )
  {
    if x != 0 
    {
      self.split( self.left(x), sp );
      if sp.count  < sp.half 
      { 
        sp.left.append_from( self, x ); 
      } else { 
        if sp.count == sp.half { sp.split_node = x; }
        sp.right.append_from( self, x );
      }
      sp.count += 1;
      self.split( self.right(x), sp );
    }
  }

  fn find_node( &self, r: &dyn Record ) -> usize
  // Returns node id of the greatest Record less than or equal to v, or zero if no such node exists.
  {
    let mut x = self.root;
    let mut result = 0;
    while x != 0
    {
      let c = self.compare( r, x );
      match c
      {
        Ordering::Greater => x = self.left( x ),
        Ordering::Less => { result = x; x = self.right( x ) },
        Ordering::Equal => { result = x; break; }
      }
    }
    result
  }

  fn insert( &mut self, r: &dyn Record )
  {
    let inserted = self.next_alloc();
    self.root = self.insert_into( self.root, Some(r) ).0;
    self.dirty = true;
    self.set_record( inserted, r );
  }

  fn insert_child( &mut self, r: &dyn Record, pnum: usize )
  {
    let inserted = self.next_alloc();
    self.root = self.insert_into( self.root, Some(r) ).0;
    self.dirty = true;
    self.set_record( inserted, r );
    self.set_child( inserted, pnum );    
  }

  fn append_child( &mut self, r: &dyn Record, pnum: usize )
  {
    let inserted = self.next_alloc();
    self.root = self.insert_into( self.root, None ).0;
    self.dirty = true;
    self.set_record( inserted, r );
    self.set_child( inserted, pnum );
  }

  fn append_from( &mut self, from: &Page, x: usize ) 
  {
    if self.parent && self.first_page == 0
    {
      self.first_page = from.child( x );
    } else {
      let inserted = self.next_alloc();
      self.root = self.insert_into( self.root, None ).0;
      let dest_off = self.rec_offset( inserted );
      let src_off = from.rec_offset( x );
      let n = self.node_size - NODE_OVERHEAD;
      for i in 0..n
      {
        self.data[ dest_off + i ] = from.data[ src_off + i ];
      }
    }
    self.dirty = true;
  }

  fn remove( &mut self, r: &dyn Record )
  {
    self.root = self.remove_from( self.root, r ).0;
    self.dirty = true;
  }

  // Node access functions.

  fn balance( &self, x: usize ) -> u8
  {
    let off = NODE_BASE + (x-1) * self.node_size;
    getbits!( self.data[off], 0, 2 )
  }

  fn set_balance( &mut self, x: usize, balance: u8 )
  {
    let off = NODE_BASE + (x-1) * self.node_size;
    setbits!( self.data[ off ], 0, 2, balance );
  } 

  fn left( &self, x: usize ) -> usize
  {
    let off = NODE_BASE + (x-1) * self.node_size;
    self.data[ off + 1 ] as usize | ( getbits!( self.data[ off ] as usize, 2, NODE_ID_BITS-8 ) << 8 )
  }

  fn right( &self, x: usize ) -> usize
  { 
    let off = NODE_BASE + (x-1) * self.node_size;
    self.data[ off + 2 ] as usize | ( getbits!( self.data[ off ] as usize, 2+NODE_ID_BITS-8, NODE_ID_BITS-8 ) << 8 )
  }

  fn set_left( &mut self, x: usize, y: usize )
  {
    let off : usize = NODE_BASE + (x-1) * self.node_size;
    self.data[ off + 1 ] = ( y & 255 ) as u8;
    setbits!( self.data[ off ], 2, NODE_ID_BITS-8, ( y >> 8 ) as u8 );
    debug_assert!( self.left( x ) == y );
  }

  fn set_right( &mut self, x: usize, y: usize )
  {
    let off : usize = NODE_BASE + (x-1) * self.node_size;
    self.data[ off + 2 ] = ( y & 255 ) as u8;
    setbits!( self.data[ off ], 2+NODE_ID_BITS-8, NODE_ID_BITS-8, ( y >> 8 ) as u8 );
    debug_assert!( self.right( x ) == y );
  }

  fn child( &self, x: usize ) -> usize
  {
    let off = NODE_BASE + x * self.node_size - PAGE_ID_SIZE;
    get( &self.data, off, PAGE_ID_SIZE ) as usize
  }

  fn set_child( &mut self, x: usize, pnum: usize )
  {
    let off = NODE_BASE + x * self.node_size - PAGE_ID_SIZE;
    set( &mut self.data, off, pnum as u64, PAGE_ID_SIZE );
  }

  fn rec_offset( &self, x:usize ) -> usize
  {
    NODE_BASE + NODE_OVERHEAD + (x-1) * self.node_size
  }

  fn set_record( &mut self, x:usize, r: &dyn Record )
  {
    let off = self.rec_offset( x );
    r.save( &mut self.data, off, !self.parent );
  }

  fn get_record( &self, x:usize, r: &mut dyn Record )
  {
    let off = self.rec_offset( x );
    r.load( &self.data, off, !self.parent );
  }

  fn compare( &self, r: &dyn Record, x:usize ) -> Ordering
  {
    let off = self.rec_offset( x );
    r.compare( &self.data, off )
  }

  fn get_key( &self, x:usize, r: &dyn Record ) -> Box<dyn Record>
  {
    let off = self.rec_offset( x );
    r.key( &self.data, off )
  }

  // Node Id Allocation.

  fn next_alloc( &mut self ) -> usize
  {
    if self.free != 0 { self.free } else { self.count + 1 }
  }

  fn alloc_node( &mut self ) -> usize
  {
    self.count += 1;
    if self.free == 0
    {
      self.alloc += 1;
      self.count
    } else {
      let result = self.free;
      self.free = self.left( self.free );
      result
    }
  }

  fn free_node( &mut self, x: usize )
  {
    self.set_left( x, self.free );
    self.free = x;
    self.count -= 1;
  }

  fn insert_into( &mut self, mut x: usize, r: Option<&dyn Record> ) -> ( usize, bool )
  {
    let mut height_increased: bool;
    if x == 0
    {
      x = self.alloc_node();
      self.set_balance( x, BALANCED );
      self.set_left( x, 0 );
      self.set_right( x, 0 );
      height_increased = true;
    } else {
      let c = match r 
      {
        Some(r) => self.compare( r, x ),
        None => Ordering::Less
      };

      if c == Ordering::Greater
      {
        let p = self.insert_into( self.left(x), r );
        self.set_left( x, p.0 );
        height_increased = p.1;
        if height_increased
        {
          let bx = self.balance( x );
          if bx == BALANCED
          {
            self.set_balance( x, LEFT_HIGHER );
          } else {
            height_increased = false;
            if bx == LEFT_HIGHER
            {
              return ( self.rotate_right( x ).0, false );
            }
            self.set_balance( x, BALANCED );
          }
        }
      } else if c == Ordering::Less {
        let p = self.insert_into( self.right(x), r );
        self.set_right( x, p.0 );
        height_increased = p.1;
        if height_increased
        {
          let bx = self.balance( x );
          if bx == BALANCED
          {
            self.set_balance( x, RIGHT_HIGHER );
          } else {
            if bx == RIGHT_HIGHER
            {
              return ( self.rotate_left( x ).0, false );
            }
            height_increased = false;
            self.set_balance( x, BALANCED );
          }
        }
      } else {
        height_increased = false; // Duplicate key
      }
    }
    ( x, height_increased )
  }

  fn rotate_right( &mut self, x: usize ) -> ( usize, bool )
  {
    // Left is 2 levels higher than Right.
    let mut height_decreased = true;
    let z = self.left( x );
    let y = self.right( z );
    let zb = self.balance( z );
    if zb != RIGHT_HIGHER // Single rotation.
    {
      self.set_right( z, x );
      self.set_left( x, y );
      if zb == BALANCED // Can only occur when deleting Records.
      {
        self.set_balance( x, LEFT_HIGHER );
        self.set_balance( z, RIGHT_HIGHER );
        height_decreased = false;
      } else { // zb = LEFT_HIGHER
        self.set_balance( x, BALANCED );
        self.set_balance( z, BALANCED );
      }
      ( z, height_decreased )
    } else { // Double rotation.
      self.set_left( x, self.right( y ) );
      self.set_right( z, self.left( y ) );
      self.set_right( y, x );
      self.set_left( y, z );
      let yb = self.balance( y );
      if yb == LEFT_HIGHER
      {
        self.set_balance( x, RIGHT_HIGHER );
        self.set_balance( z, BALANCED );
      } else if yb == BALANCED {
        self.set_balance( x, BALANCED );
        self.set_balance( z, BALANCED );
      } else { // yb == RIGHT_HIGHER
        self.set_balance( x, BALANCED );
        self.set_balance( z, LEFT_HIGHER );
      }
      self.set_balance( y, BALANCED );
      ( y, height_decreased )
    }
  }

  fn rotate_left( &mut self, x: usize ) -> ( usize, bool )
  {
    // Right is 2 levels higher than Left.
    let mut height_decreased = true;
    let z = self.right( x );
    let y = self.left( z );
    let zb = self.balance( z );
    if zb != LEFT_HIGHER // Single rotation.
    {
      self.set_left( z, x );
      self.set_right( x, y );
      if zb == BALANCED // Can only occur when deleting Records.
      {
        self.set_balance( x, RIGHT_HIGHER );
        self.set_balance( z, LEFT_HIGHER );
        height_decreased = false;
      } else { // zb = RIGHT_HIGHER
        self.set_balance( x, BALANCED );
        self.set_balance( z, BALANCED );
      }
      (z, height_decreased )
    } else { // Double rotation
      self.set_right( x, self.left( y ) );
      self.set_left( z, self.right( y ) );
      self.set_left( y, x );
      self.set_right( y, z );
      let yb = self.balance( y );
      if yb == RIGHT_HIGHER
      {
        self.set_balance( x, LEFT_HIGHER );
        self.set_balance( z, BALANCED );
      } else if yb == BALANCED {
        self.set_balance( x, BALANCED );
        self.set_balance( z, BALANCED );
      } else { // yb == LEFT_HIGHER
        self.set_balance( x, BALANCED );
        self.set_balance( z, RIGHT_HIGHER );
      }
      self.set_balance( y, BALANCED );
      ( y, height_decreased )
    }
  }

  fn remove_from( &mut self, mut x: usize, r: &dyn Record  ) -> ( usize, bool ) // out bool heightDecreased
  {
    if x == 0 // key not found.
    {
      return ( x, false );
    }
    let mut height_decreased: bool = true;
    let compare = self.compare( r, x );
    if compare == Ordering::Equal
    {
      let deleted = x;
      if self.left( x ) == 0
      {
        x = self.right( x );
      } else if self.right( x ) == 0 {
        x = self.left( x );
      } else {
        // Remove the smallest element in the right sub-tree and substitute it for x.
        let t = self.remove_least( self.right(deleted) );
        let right = t.0;
        x = t.1;
        height_decreased = t.2;

        self.set_left( x, self.left( deleted ) );
        self.set_right( x, right );
        self.set_balance( x, self.balance( deleted ) );
        if height_decreased
        {
          if self.balance( x ) == LEFT_HIGHER
          {
            let rr = self.rotate_right( x );
            x = rr.0;
            height_decreased = rr.1;
          } else if self.balance( x ) == RIGHT_HIGHER {
            self.set_balance( x, BALANCED );
          } else {
            self.set_balance( x, LEFT_HIGHER );
            height_decreased = false;
          }
        }
      }
      self.free_node( deleted );
    } else if compare == Ordering::Greater {
      let rem = self.remove_from( self.left( x ), r );
      self.set_left( x, rem.0 );
      height_decreased = rem.1;
      if height_decreased
      {
        let xb = self.balance( x );
        if xb == RIGHT_HIGHER
        {
          return self.rotate_left( x );
        }
        if xb == LEFT_HIGHER
        {
          self.set_balance( x, BALANCED );
        } else {
          self.set_balance( x, RIGHT_HIGHER );
          height_decreased = false;
        }
      }
    } else {
      let rem = self.remove_from( self.right(x), r );
      self.set_right( x, rem.0 );
      height_decreased = rem.1;
      if height_decreased
      { 
        let xb = self.balance( x );
        if xb == LEFT_HIGHER
        {
          return self.rotate_right( x );
        }
        if self.balance( x ) == RIGHT_HIGHER
        {
          self.set_balance( x, BALANCED );
        } else {
          self.set_balance( x, LEFT_HIGHER );
          height_decreased = false;
        }
      }
    }
    ( x, height_decreased )
  }

  // Returns root of tree, removed node and height_decreased.
  fn remove_least( &mut self, x: usize ) -> ( usize, usize, bool )
  {
    if self.left(x) == 0
    {
      ( self.right( x ), x, true )
    } else {
      let t = self.remove_least( self.left(x) );
      self.set_left( x, t.0 );
      let least = t.1;
      let mut height_decreased = t.2;
      if height_decreased
      {
        let xb = self.balance( x );
        if xb == RIGHT_HIGHER
        {
          let rl = self.rotate_left( x );
          return ( rl.0, least, rl.1 );
        }
        if xb == LEFT_HIGHER
        {
          self.set_balance( x, BALANCED );
        } else {
          self.set_balance( x, RIGHT_HIGHER );
          height_decreased = false;
        }
      }
      ( x, least, height_decreased )
    }
  }
} // end impl Page

// *********************************************************************

struct ParentInfo<'a>
{
  pnum: usize,
  parent: Option<&'a ParentInfo<'a>>
}  

struct Split
{
  count: usize,
  half: usize,
  split_node: usize,
  left: Page,
  right: Page
}  

impl Split
{
  fn new( p: &Page ) -> Split
  {
    let mut result =
    Split
    {
      count:0,
      half: p.count/2,
      split_node: 0,
      left: p.new_page(),
      right: p.new_page()
    };
    result.left.first_page = p.first_page; 
    p.split( p.root, &mut result );
    result
  }
}

// *********************************************************************

impl <'stg,'file,'start> Cursor <'stg,'file,'start>
{
  fn new( start: &'start dyn Record, ixf: &'file mut File<'stg> ) -> Cursor<'stg,'file,'start>
  {
    Cursor{ stk: Stack::new(start), ixf }
  }

  pub fn reset( &mut self, start: &'start dyn Record )
  {
    self.stk.reset( start ); 
  }

  pub fn next( &mut self, r: &mut dyn Record ) -> bool
  {
    self.stk.next( self.ixf, r )
  }

  pub fn prev( &mut self, r: &mut dyn Record ) -> bool
  {
    self.stk.prev( self.ixf, r )
  }
}

struct Stack <'a>
{
  len: usize,
  arr: [usize;50],
  start: &'a dyn Record,
  seeking: bool,
  state: u8
}

impl <'a> Stack <'a>
{
  /// Create a new Stack with specified start key.
  pub fn new( start: &'a dyn Record ) -> Stack
  {
    Stack{ arr:[0;50], start, len:0, seeking:false, state:0 }
  }

  /// Reset a Stack with specified start key.
  pub fn reset( &mut self, start: &'a dyn Record )
  {
    self.state = 0;
    self.start = start;
  }

  /// Fetch next Record, result indicates success.
  pub fn next( &mut self, ixf: &mut File, r: &mut dyn Record ) -> bool
  {
    if self.state != 2
    {
      self.state = 2;
      self.seeking = true;
      self.len = 0;
      self.add_page_right( ixf, 0 );
    }
    loop
    {
      match self.pop()
      {
        None => { self.state = 0; return false },
        Some( ( pnum, x ) ) =>
        {     
          if x == 0
          {
            self.add_page_right( ixf, pnum );
          } else {
            let p = &ixf.pages[ pnum ];
            self.add_right( p, pnum, p.left( x ) );
            if p.parent 
            {
              let cp = p.child( x );
              self.add_page_right( ixf, cp ); 
            } else {
              self.seeking = false;
              p.get_record( x, r );
              return true;
            }
          }                   
        }              
      }
    }
  }

  /// Fetch previous Record, result indicates success.
  pub fn prev( &mut self, ixf: &mut File, r: &mut dyn Record ) -> bool
  {
    if self.state != 1
    {
      self.state = 1;
      self.seeking = true;
      self.len = 0;
      self.add_page_left( ixf, 0 );
    }
    loop
    {
      match self.pop()
      {
        None => { self.state = 0; return false },
        Some( ( pnum, x ) ) =>
        {     
          let p = &ixf.pages[ pnum ];
          self.add_left( p, pnum, p.right( x ) );
          if p.parent 
          {
            let cp = p.child( x );
            self.add_page_left( ixf, cp ); 
          } else {
            self.seeking = false;
            p.get_record( x, r );
            return true;
          }                   
        }              
      }
    }
  }

  fn push( &mut self, pnum: usize, x: usize )
  {
    self.arr[ self.len ] = ( pnum << NODE_ID_BITS ) + x;
    self.len += 1;
  }

  fn pop( &mut self ) -> Option< (usize,usize) >
  {
    if self.len == 0
    {
      None
    } else {
      self.len -= 1;
      let v = self.arr[ self.len ];
      Some( ( v >> NODE_ID_BITS, getbits!( v, 0, NODE_ID_BITS ) ) )
    }
  }

  fn add_left( &mut self, p: &Page, pnum: usize, mut x: usize )
  {
    while x != 0
    {
      self.push( pnum, x );
      x = p.left( x );
    }
  }

  fn add_right( &mut self, p: &Page, pnum: usize, mut x: usize )
  {
    while x != 0
    {
      self.push( pnum, x );
      x = p.right( x );
    }
  }

  fn seek_left( &mut self, p: &Page, pnum: usize, x:usize ) -> bool
  // Returns true if a node is found which is <= start.
  // This is used to decide whether the the preceding child page is added.
  {
    if x == 0 { return false; }
    let c = p.compare( self.start, x );
    match c
    {
      Ordering::Greater =>
      {
        self.push( pnum, x );
        self.seek_left( p, pnum, p.left( x ) )
      }
      Ordering::Equal => 
      {
        self.push( pnum, x );
        true
      }
      Ordering::Less =>
      {
        if !self.seek_left( p, pnum, p.right( x ) ) && p.parent
        {
          self.push( pnum, x );
        }
        true
      }
    }
  }

  fn seek_right( &mut self, p: &Page, pnum: usize, mut x:usize )
  {
    while x != 0
    {
      let c = p.compare( self.start, x );
      match c
      {
        Ordering::Less =>
        {
          self.push( pnum, x );
          x = p.right( x );
        }
        Ordering::Equal => 
        {
          self.push( pnum, x );
          break;
        }
        Ordering::Greater =>
        {
          x = p.left( x );
        }
      }
    }
  }

  fn add_page_right( &mut self, ixf:&mut File, pnum:usize )
  {
    let p = ixf.load_page( pnum );
    if p.parent { self.push( p.first_page, 0 ); }
    let root = p.root;
    if self.seeking 
    {
      self.seek_right( p, pnum, root );
    } else { 
      self.add_right( p, pnum, root ); 
    }
  }

  fn add_page_left( &mut self, ixf:&mut File, mut pnum:usize )
  {
    loop
    {
      let p = ixf.load_page( pnum );
      let root = p.root;
      if self.seeking 
      {
        if self.seek_left( p, pnum, root ) { return; }
      } else { 
        self.add_left( p, pnum, root ); 
      }
      if !p.parent { return; }
      pnum = p.first_page;
    }
  }
} // end impl Stack
