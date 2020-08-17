use std::cmp::Ordering;

pub trait Record
{
  fn save( &self, data:&mut [u8], off: usize );
  fn load( &mut self, data: &[u8], off: usize );
  fn compare( &self, data: &[u8], off: usize ) -> Ordering;
  fn make( &self, data:&[u8], off: usize ) -> Box<dyn Record>;
}

pub trait BackingStorage
{
  fn read_page( &self, pnum: usize, data: &mut[u8] );
}

pub trait Consumer
{
  // return false to abort processing.
  fn process( &self, data: &[u8], off:usize ) -> bool;
}

struct ParentInfo<'a>
{
  pnum: usize,
  parent: Option<&'a ParentInfo<'a>>
}  

pub struct IndexFile<'a>
{
  pub pages: Vec<IndexPage>,
  pub rec_size: usize,
  store: &'a dyn BackingStorage
}

impl <'a> IndexFile<'a>
{
  pub fn new( rec_size: usize, store: &'a dyn BackingStorage, pcount: usize ) -> IndexFile<'a>
  {
    let mut result = IndexFile
    { 
      pages: Vec::with_capacity( pcount ), 
      rec_size, 
      store 
    };

    if pcount == 0
    {
      result.pages.push( IndexPage::new( rec_size, false, vec![0;PAGE_SIZE] ) );
    } else {
      for _i in 0..pcount
      {
        result.pages.push( IndexPage::blank() );
      }
    }
    result    
  }

  pub fn insert( &mut self, r: &dyn Record )
  {
    self.insert_leaf( 0, r, None );
  }

  pub fn fetch( &mut self, start: &dyn Record, c: &dyn Consumer )
  {
    self.fetch_page( 0, start, c );
  }

  pub fn remove( &mut self, r: &dyn Record )
  {
    self.load_page( 0 );
    let mut p = &mut self.pages[ 0 ];
    while p.parent
    {
      let x = p.find_node( r );
      let cp = if x == 0 { p.first_page } else { p.get_child( x ) };
      self.load_page( cp );
      p = &mut self.pages[ cp ];
    }
    p.remove( r );
  }

  fn load_page( &mut self, pnum: usize )
  {
    if self.pages[ pnum ].data.is_empty()
    {
      let mut data = vec![ 0; PAGE_SIZE ];
      self.store.read_page( pnum, &mut data );
      let parent = data[0] & 1 != 0;
      self.pages[ pnum ] = IndexPage::new( self.rec_size, parent, data );
    }
  }

  fn insert_leaf( &mut self, pnum: usize, r: &dyn Record, pi: Option<&ParentInfo> )
  {
    self.load_page( pnum );
    let p = &mut self.pages[ pnum ];
    if p.parent
    {
      // Look for child page to insert into.
      let x = p.find_node( r );
      let cp = if x == 0 { p.first_page } else { p.get_child( x ) };
      self.insert_leaf( cp, r, Some(&ParentInfo{ pnum, parent:pi }) );
    } else if !p.full() {
      p.insert( r );
    }  else {
      // Page is full, divide it into left and right.
      let sp = Split::new( p, self.rec_size );
      let sk = &*p.get_key( sp.split_node, r );

      // Could insert r into left or right here.

      let pnum2 = self.pages.len();
      self.pages.push( sp.right );
      match pi 
      {
        None =>
        {
          // New root page needed.
          let mut new_root = IndexPage::new( self.rec_size, true, vec![ 0; PAGE_SIZE ] );
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

      let mut sp = Split::new( p, self.rec_size );
      let sk = &*p.get_key( sp.split_node, r );

      // Insert into either left or right.
      let c = p.compare( r, sp.split_node );
      if c == Ordering::Less 
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
          let mut new_root = IndexPage::new( self.rec_size, true, vec![ 0; PAGE_SIZE ] );
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

  fn fetch_page( &mut self, pnum: usize, start: &dyn Record, con: &dyn Consumer ) -> bool
  {
    self.load_page( pnum );
    let p = &self.pages[ pnum ];
    let root = p.root;    
    if p.parent 
    { 
      let first_page = p.first_page;
      let first_node = p.first_node();
      if p.compare( start, first_node ) == Ordering::Less &&
        !self.fetch_page( first_page, start, con ) { return false; } 
    }
    self.fetch_node( pnum, root, start, con )
  }

  fn fetch_node( &mut self, pnum: usize, x:usize, start: &dyn Record, con: &dyn Consumer ) -> bool
  {
    if x == 0 { return true; }
    self.load_page( pnum );
    let p = &self.pages[ pnum ];
    let parent = p.parent;
    let cp = if parent { p.get_child( x ) } else { 0 };
    let left = p.get_left( x );
    let right = p.get_right( x );

    let c = p.compare( start, x );
    if c == Ordering::Less && !self.fetch_node( pnum, left, start, con ) { return false; } 

    if parent 
    {
      if !self.fetch_page( cp, start, con ) { return false; }
    }
    else if c == Ordering::Less || c == Ordering::Equal
    {
      let p = &self.pages[ pnum ];
      if !con.process( &p.data, p.rec_offset( x ) ) { return false; }
    }

    self.fetch_node( pnum, right, start, con )
  }
} // end impl IndexFile

// *********************************************************************

pub struct IndexPage
{
  pub data: Vec<u8>, // Data storage.
  root: usize,       // Root node.
  pub count: usize,  // Number of Records currently stored.
  node_alloc: usize, // Number of Nodes currently allocated.
  free: usize,       // First Free node.
  node_base: usize,  // Could be calculated dynamically ( or become a constant if store first_page after nodes ).
  node_size: usize,  // Number of bytes required for each node.
  max_node: usize,   // Maximum number of nodes ( constrained by PAGE_SIZE ).

  first_page: usize, // First child page ( for a non-leaf page ).
  pub parent: bool,      // Is page a parent page?
  dirty: bool,       // Does page need to be saved to disk?
}

const PAGE_SIZE : usize = 0x1000; // Good possibilities are 0x1000, 0x2000 and 0x4000.
const NODE_OVERHEAD : usize = 3; // Size of Balance,Left,Right in a Node ( 2 + 2 x 11 = 24 bits  needs 3 bytes ).
const FIXED_HEADER : usize = 6; // 45 bits ( 1 + 4 x 11 ) needs 6 bytes.
const PAGE_ID_SIZE : usize = 6; // Number of bytes used to store a page number.

const BALANCED : i8 = 0;
const LEFT_HIGHER : i8 = -1;
const RIGHT_HIGHER : i8 = 1;
const MAX_NODE : usize = 10; // 2047; // Node ids are 11 bits.

impl IndexPage
{
  fn blank() -> IndexPage
  {
    IndexPage
    {
      data: Vec::new(),
      root: 0, 
      count: 0,
      node_alloc: 0,
      free: 0,
      node_size: 0,
      node_base: 0,
      max_node: 0,
      first_page: 0,
      parent: false,
      dirty: false,
    }
  }

  fn new( rec_size:usize, parent:bool, data: Vec<u8> ) -> IndexPage
  {
    let node_size = NODE_OVERHEAD + rec_size + if parent {PAGE_ID_SIZE} else {0};
    let node_base = FIXED_HEADER + if parent {PAGE_ID_SIZE} else {0};
    let mut max_node = ( PAGE_SIZE - ( node_base + node_size ) ) / node_size;
    if max_node > MAX_NODE { max_node = MAX_NODE; } 

    let u = get( &data, 0, FIXED_HEADER );
    let root = ( ( u >> 1 ) & 0x7ff ) as usize;
    let count = ( ( u >> 12 ) & 0x7ff ) as usize;
    let free = ( ( u >> 23 ) & 0x7ff ) as usize;
    let node_alloc = ( ( u >> 34 ) & 0x7ff ) as usize;
    let first_page = if parent { get( &data, FIXED_HEADER, PAGE_ID_SIZE ) } else {0} as usize;

    IndexPage
    {
      data,
      root, 
      count,
      node_alloc,
      free,
      node_size,
      node_base,
      max_node,
      first_page,
      parent,
      dirty: false,
    }
  }

  pub fn size( &self ) -> usize
  {
    self.node_base + self.node_alloc * self.node_size
  }

  fn full( &self ) -> bool
  {
    self.free == 0 && self.node_alloc == self.max_node
  }

  fn write_header(&mut self) // Called just before page is saved to file.
  { 
    let u  = 
    if self.parent {1} else {0}
    | ( ( self.root as u64 ) << 1 )
    | ( ( self.count as u64 ) << 12 )
    | ( ( self.free as u64 ) << 23 )
    | ( ( self.node_alloc as u64 ) << 34 );

    set( &mut self.data, 0, u, FIXED_HEADER );
    if self.parent
    { 
      set( &mut self.data, FIXED_HEADER, self.first_page as u64, PAGE_ID_SIZE );
    }
  }

  fn split( &self, x:usize, sp:&mut Split )
  {
    if x != 0 
    {
      self.split( self.get_left(x), sp );
      if sp.count  < sp.half 
      { 
        sp.left.append_from( self, x ); 
      } else { 
        if sp.count == sp.half { sp.split_node = x; }
        sp.right.append_from( self, x );
      }
      sp.count += 1;
      self.split( self.get_right(x), sp );
    }
  }

  fn first_node( &self ) -> usize
  {
    let mut result = self.root;
    loop
    {
      let x = self.get_left( result );
      if x == 0 { break; }
      result = x;
    }
    result
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
        Ordering::Less => x = self.get_left( x ),
        Ordering::Greater => { result = x; x = self.get_right( x ) },
        Ordering::Equal => { result = x; break; }
      }
    }
    result
  }

  fn insert( &mut self, r: &dyn Record )
  {
    let inserted = self.next_alloc();
    self.root = self.insert0( self.root, Some(r) ).0;
    self.dirty = true;
    self.set_record( inserted, r );
    self.write_header();
  }

  fn insert_child( &mut self, r: &dyn Record, pnum: usize )
  {
    let inserted = self.next_alloc();
    self.root = self.insert0( self.root, Some(r) ).0;
    self.dirty = true;
    self.set_record( inserted, r );
    self.set_child( inserted, pnum );    
  }

  fn append_child( &mut self, r: &dyn Record, pnum: usize )
  {
    let inserted = self.next_alloc();
    self.root = self.insert0( self.root, None ).0;
    self.dirty = true;
    self.set_record( inserted, r );
    self.set_child( inserted, pnum );
  }

  fn append_from( &mut self, from: &IndexPage, x: usize ) 
  {
    if self.parent && self.first_page == 0
    {
      self.first_page = from.get_child( x );
    } else {
      let inserted = self.next_alloc();
      self.root = self.insert0( self.root, None ).0;
      let dest_off = self.rec_offset( inserted );
      let src_off = from.rec_offset( x );
      let n = self.node_size - NODE_OVERHEAD;
      for i in 0..n
      {
        self.data[ dest_off + i ] = from.data[ src_off + i ];
      }
    }
  }

  fn remove( &mut self, r: &dyn Record )
  {
    self.root = self.remove_from( self.root, r ).0;
  }

  // Node access functions.

  fn get_balance( &self, x: usize ) -> i8
  {
    let off = self.node_base + (x-1) * self.node_size;
    ( self.data[ off ] & 3 ) as i8 - 1 // Extract the low two bits.
  }

  fn set_balance( &mut self, x: usize, balance: i8 ) // balance is in range -1 .. +1
  {
    let off = self.node_base + (x-1) * self.node_size;
    self.data[ off ] = ( balance + 1 ) as u8 | ( self.data[ off ] & 0xfc );
  } 

  fn get_left( &self, x: usize ) -> usize
  { 
    let off = self.node_base + (x-1) * self.node_size;
    self.data[ off + 1 ] as usize 
    | ( ( self.data[ off ] as usize & 28 ) << 6 ) // 28 = 7 << 2; adds bits 2..4 from Data[ off ]
  }

  fn get_right( &self, x: usize ) -> usize
  { 
    let off = self.node_base + (x-1) * self.node_size;
    self.data[ off + 2 ] as usize 
      | ( ( self.data[ off ] as usize & 224 ) << 3 ) // 224 = 7 << 5; adds in bits 5..7 of Data[ off ]
  }

  fn set_left( &mut self, x: usize, y: usize )
  {
    const MASK : u8 = 28; // 28 = 7 << 2
    let off : usize = self.node_base + (x-1) * self.node_size;
    self.data[ off + 1 ] = ( y & 255 ) as u8;
    self.data[ off ] = ( self.data[ off ] & ( 255 - MASK ) )
      | ( ( y >> 6 ) as u8 & MASK );
    // if self.get_left( x ) != y { panic!("set_left"); }
  }

  fn set_right( &mut self, x: usize, y: usize )
  {
    const MASK : u8 = 224; // 224 = 7 << 5
    let off = self.node_base + (x-1) * self.node_size;
    self.data[ off + 2 ] = ( y & 255 ) as u8;
    self.data[ off] = ( self.data[ off ] & ( 255 - MASK ) ) 
      | ( ( y >> 3 ) as u8 & MASK );
  }

  fn get_child( &self, x: usize ) -> usize
  {
    let off = self.node_base + x * self.node_size - PAGE_ID_SIZE;
    get( &self.data, off, PAGE_ID_SIZE ) as usize
  }

  fn set_child( &mut self, x: usize, pnum: usize )
  {
    let off = self.node_base + x * self.node_size - PAGE_ID_SIZE;
    set( &mut self.data, off, pnum as u64, PAGE_ID_SIZE );
  }

  fn rec_offset( &self, x:usize ) -> usize
  {
    self.node_base + NODE_OVERHEAD + (x-1) * self.node_size
  }

  fn set_record( &mut self, x:usize, r: &dyn Record )
  {
    let off = self.rec_offset( x );
    r.save( &mut self.data, off );
  }

  fn compare( &self, r: &dyn Record, x:usize ) -> Ordering
  {
    let off = self.rec_offset( x );
    r.compare( &self.data, off )
  }

  fn get_key( &self, x:usize, r: &dyn Record ) -> Box<dyn Record>
  {
    let off = self.rec_offset( x );
    r.make( &self.data, off )
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
      self.node_alloc += 1;
      self.count
    } else {
      let result = self.free;
      self.free = self.get_left( self.free );
      result
    }
  }

  fn free_node( &mut self, x: usize )
  {
    self.set_left( x, self.free );
    self.free = x;
    self.count -= 1;
  }

  fn insert0( &mut self, mut x: usize, r: Option<&dyn Record> ) -> ( usize, bool )
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
        None => Ordering::Greater
      };

      if c == Ordering::Less
      {
        let p = self.insert0( self.get_left(x), r );
        self.set_left( x, p.0 );
        height_increased = p.1;
        if height_increased
        {
          let bx = self.get_balance( x );
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
      } else {
        let p = self.insert0( self.get_right(x), r );
        self.set_right( x, p.0 );
        height_increased = p.1;
        if height_increased
        {
          let bx = self.get_balance( x );
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
      }
    }
    ( x, height_increased )
  }

  fn rotate_right( &mut self, x: usize ) -> ( usize, bool )
  {
    // Left is 2 levels higher than Right.
    let mut height_decreased = true;
    let z = self.get_left( x );
    let y = self.get_right( z );
    let zb = self.get_balance( z );
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
      self.set_left( x, self.get_right( y ) );
      self.set_right( z, self.get_left( y ) );
      self.set_right( y, x );
      self.set_left( y, z );
      let yb = self.get_balance( y );
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
    let z = self.get_right( x );
    let y = self.get_left( z );
    let zb = self.get_balance( z );
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
      self.set_right( x, self.get_left( y ) );
      self.set_left( z, self.get_right( y ) );
      self.set_left( y, x );
      self.set_right( y, z );
      let yb = self.get_balance( y );
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
      if self.get_left( x ) == 0
      {
        x = self.get_right( x );
      } else if self.get_right( x ) == 0 {
        x = self.get_left( x );
      } else {
        // Remove the smallest element in the right sub-tree and substitute it for x.
        let t = self.remove_least( self.get_right(deleted) );
        let right = t.0;
        x = t.1;
        height_decreased = t.2;

        self.set_left( x, self.get_left( deleted ) );
        self.set_right( x, right );
        self.set_balance( x, self.get_balance( deleted ) );
        if height_decreased
        {
          if self.get_balance( x ) == LEFT_HIGHER
          {
            let rr = self.rotate_right( x );
            x = rr.0;
            height_decreased = rr.1;
          } else if self.get_balance( x ) == RIGHT_HIGHER {
            self.set_balance( x, BALANCED );
          } else {
            self.set_balance( x, LEFT_HIGHER );
            height_decreased = false;
          }
        }
      }
      self.free_node( deleted );
    } else if compare == Ordering::Less {
      let rem = self.remove_from( self.get_left( x ), r );
      self.set_left( x, rem.0 );
      height_decreased = rem.1;
      if height_decreased
      {
        let xb = self.get_balance( x );
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
      let rem = self.remove_from( self.get_right(x), r );
      self.set_right( x, rem.0 );
      height_decreased = rem.1;
      if height_decreased
      { 
        let xb = self.get_balance( x );
        if xb == LEFT_HIGHER
        {
          return self.rotate_right( x );
        }
        if self.get_balance( x ) == RIGHT_HIGHER
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
    if self.get_left(x) == 0
    {
      ( self.get_right( x ), x, true )
    } else {
      let t = self.remove_least( self.get_left(x) );
      self.set_left( x, t.0 );
      let least = t.1;
      let mut height_decreased = t.2;
      if height_decreased
      {
        let xb = self.get_balance( x );
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
} // end impl IndexPage

struct Split
{
  count: usize,
  half: usize,
  split_node: usize,
  left: IndexPage,
  right: IndexPage
}  

impl Split
{
  fn new( p: &IndexPage, rec_size: usize ) -> Split
  {
    let mut result =
    Split
    {
      count:0,
      half: p.count/2,
      split_node: 0,
      left: IndexPage::new( rec_size, p.parent, vec![ 0; PAGE_SIZE ] ),
      right: IndexPage::new( rec_size, p.parent, vec![ 0; PAGE_SIZE ] ),
    };
    result.left.first_page = p.first_page; 
    p.split( p.root, &mut result );
    result
  }
}

// Extract unsigned value of n bytes from data[off].
pub fn get( data: &[u8], off: usize, n: usize ) -> u64
{
  let mut x = 0;
  for i in 0..n
  {
    x = ( x << 8 ) + data[ off + n - i - 1 ] as u64;
  }
  x
}

// Store unsigned value of n bytes to data[off].
pub fn set( data: &mut[u8], off: usize, mut val:u64, n: usize )
{
  for i in 0..n
  {
    data[ off + i ] = ( val & 255 ) as u8;
    val >>= 8;
  }
}
