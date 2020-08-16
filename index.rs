pub trait Record
{
  fn size( &self ) -> usize;
  fn save( &self, data:&mut [u8], off: usize );
  fn load( &mut self, data:&[u8], off: usize );
  fn compare( &self, data: &[u8], off: usize ) -> i8;
  fn make( &self, data:&[u8], off: usize ) -> Box<dyn Record>;
  fn dump( &self );
}

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
  left: IndexPage,
  right: IndexPage
}  

impl Split
{
  fn new( p: &IndexPage ) -> Split
  {
    let mut result =
    Split
    {
      count:0,
      half: p.count/2,
      split_node: 0,
      left: IndexPage::new( p.rec_size(), p.parent, vec![ 0; PAGE_SIZE ] ),
      right: IndexPage::new( p.rec_size(), p.parent, vec![ 0; PAGE_SIZE ] ),
    };
    result.left.first_page = p.first_page; 
    p.split( p.root, &mut result );
    result
  }
}

pub struct IndexFile
{
  pages: Vec<IndexPage>
}

impl IndexFile
{
  pub fn new() -> IndexFile
  {
    IndexFile{ pages: Vec::new() } 
  }

  pub fn insert( &mut self, r: &dyn Record )
  {
    if self.pages.is_empty()
    {
      // Create the root page.
      let data = vec![ 0; PAGE_SIZE ];
      let root_page = IndexPage::new( r.size(), false, data );
      self.pages.push( root_page );
    }
    self.insert_leaf( 0, r, None );
  }

  fn insert_leaf( &mut self, pnum: usize, r: &dyn Record, pi: Option<&ParentInfo> )
  {
    let p = &mut self.pages[ pnum ];
    if p.parent
    {
      // Look for child page to insert into.
      let x = p.find_node( r );
      let cp = if x == 0 { p.first_page } else { p.get_child( x ) };
      self.insert_leaf( cp, r, Some(&ParentInfo{ pnum, parent:pi }) );
    } else {
      if !p.full()
      {
        p.insert( r );
      }  else {
        // Page is full, divide it into left and right.
        let sc = Split::new( p );
        let sk = p.get_key( sc.split_node, r );

        // Could insert r into left or right here.

        let pnum2 = self.pages.len();
        self.pages.push( sc.right );
        match pi 
        {
          None =>
          {
            // New root page needed.
            let mut new_root = IndexPage::new( r.size(), true, vec![ 0; PAGE_SIZE ] );
            new_root.first_page = self.pages.len();
            self.pages.push( sc.left );
            self.pages[ 0 ] = new_root;
            self.append_page( 0, pnum2, &*sk );
          },
          Some( parent ) =>
          {  
            self.pages[ pnum ] = sc.left;
            self.insert_page( parent, pnum2, &*sk );
          }
        }
        self.insert( r ); // Could be avoided by inserting into left or right above.
      }
    }
  } 

  fn insert_page( &mut self, into: &ParentInfo, pnum: usize, k:&dyn Record )
  {
    let p = &mut self.pages[ into.pnum ];
    // Need to check if page is full.
    if !p.full() 
    {
      p.insert_child( k, pnum );
    } else {
      // Split the parent page.
      let mut sc = Split::new( p );

      let k2 = p.get_key( sc.split_node, k );

      // Insert into either left or right.
      let c = p.compare( sc.split_node, k );
      if c > 0
      {
        sc.right.insert_child( k, pnum );
      } else {
        sc.left.insert_child( k, pnum );
      }

      let pnum2 = self.pages.len();
      self.pages.push( sc.right );
     
      match into.parent
      {
        None =>
        {
          // New root page needed.
          let mut new_root = IndexPage::new( k.size(), true, vec![ 0; PAGE_SIZE ] );
          new_root.first_page = self.pages.len();
          self.pages.push( sc.left );
          self.pages[ 0 ] = new_root;
          self.append_page( 0, pnum2, &*k2 );
        },
        Some(parent) =>
        {  
          self.pages[ pnum ] = sc.left;
          self.insert_page( parent, pnum2, &*k2 );
        }
      }
    }   
  }

  fn append_page( &mut self, into: usize, pnum: usize, k:&dyn Record )
  {
    let p = &mut self.pages[ into ];
    p.append_child( k, pnum );
  }

  pub fn remove( &mut self, r: &dyn Record )
  {
    let mut p = &mut self.pages[ 0 ];
    while p.parent
    {
      let x = p.find_node( r );
      let cp = if x == 0 { p.first_page } else { p.get_child( x ) };
      p = &mut self.pages[ cp ];
    }
    p.remove( r );
  }
  
  pub fn dump( &self, r: &mut dyn Record )
  {
    println!( "IndexFile dump, page count={}", self.pages.len() );
    self.dump0( 0, r );
    println!( "end IndexFile dump" );
  }

  fn dump0(  &self, pnum: usize, r: &mut dyn Record )
  {
    println!( "IndexFile dump Page={} ", pnum );
    self.pages[ pnum ].dump( r, self );
  }
}

// *********************************************************************

struct IndexPage
{
  data: Vec<u8>,     // Data storage.
  root: usize,       // Root node.
  count: usize,      // Number of Records currently stored.
  node_alloc: usize, // Number of Nodes currently allocated.
  free: usize,       // First Free node.
  node_base: usize,  // Could be calculated dynamically.
  node_size: usize,  // Number of bytes required for each node.
  max_node: usize,   // Maximum number of nodes ( constrained by PageSize ).

  first_page: usize, // First child page ( for a non-leaf page ).
  parent: bool,      // Is page a parent page?
  saved: bool,       // Has page been saved to disk?
}

const PAGE_SIZE : usize = 0x1000; // Good possibilities are 0x1000, 0x2000 and 0x4000.
const NODE_OVERHEAD : usize = 3; // Size of Balance,Left,Right in a Node ( 2 + 2 x 11 = 24 bits  needs 3 bytes ).
const FIXED_HEADER : usize = 6; // 45 bits ( 1 + 4 x 11 ) needs 6 bytes.
const PAGE_ID_SIZE : usize = 6; // Number of bytes used to store a page number.

const BALANCED : i8 = 0;
const LEFT_HIGHER : i8 = -1;
const RIGHT_HIGHER : i8 = 1;

impl IndexPage
{
  fn new( rec_size:usize, parent:bool, data: Vec<u8> ) -> IndexPage
  {
    let node_size = NODE_OVERHEAD + rec_size + if parent {PAGE_ID_SIZE} else {0};
    let node_base = FIXED_HEADER + if parent {PAGE_ID_SIZE} else {0};
    // let mut max_node = ( PAGE_SIZE - ( node_base + node_size ) ) / node_size;
    // if max_node > 2047 { max_node = 2047; } // Node ids are 11 bits.
    let max_node = 8;

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
      saved: true,
    }
  }

  fn full( &self ) -> bool
  {
    self.free == 0 && self.node_alloc == self.max_node
  }

  fn size( &self ) -> usize
  {
    self.node_base + self.node_alloc * self.node_size
  }

  fn rec_size( &self ) -> usize
  {
    self.node_size - NODE_OVERHEAD - if self.parent {PAGE_ID_SIZE} else {0}
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

  fn split( &self, x:usize, sc:&mut Split )
  {
    if x != 0 
    {
      self.split( self.get_left(x), sc );
      if sc.count  < sc.half 
      { 
        sc.left.append( self, x ); 
      } else { 
        if sc.count == sc.half { sc.split_node = x; }
        sc.right.append( self, x );
      }
      sc.count += 1;
      self.split( self.get_right(x), sc );
    }
  }

  fn find_node( &self, r: &dyn Record ) -> usize
  // Returns node id of the greatest Record less than or equal to v, or zero if no such node exists.
  {
    let mut x = self.root;
    let mut result = 0;
    while x != 0
    {
      let c = self.compare( x, r );
      if c < 0
      {
        x = self.get_left( x );
      } else if c > 0 {
        result = x;
        x = self.get_right( x );
      } else { // c == 0
        result = x;
        break;
      }
    }
    result
  }

  fn insert( &mut self, r: &dyn Record )
  {
    let inserted = self.next_alloc();
    self.root = self.insert0( self.root, Some(r) ).0;
    self.saved = false;
    self.set_record( inserted, r );
    self.write_header();
  }

  fn insert_child( &mut self, r: &dyn Record, pnum: usize )
  {
    let inserted = self.next_alloc();
    self.root = self.insert0( self.root, Some(r) ).0;
    self.saved = false;
    self.set_record( inserted, r );
    self.set_child( inserted, pnum );    
  }

  fn append_child( &mut self, r: &dyn Record, pnum: usize )
  {
    let inserted = self.next_alloc();
    self.root = self.insert0( self.root, None ).0;
    self.saved = false;
    self.set_record( inserted, r );
    self.set_child( inserted, pnum );
  }

  fn append( &mut self, from: &IndexPage, x: usize ) 
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
    self.root = self.remove0( self.root, r ).0;
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

  fn get_record( &self, x:usize, r: &mut dyn Record )
  { 
    let off = self.rec_offset( x );
    r.load( &self.data, off );
  }  

  fn compare( &self, x: usize, r: &dyn Record ) -> i8
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
        Some(r) => self.compare( x, r ),
        None => 1
      };

      if c < 0
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
      } else if c > 0 {
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
      } else { 
        // compare == 0, should not happen, keys should be unique with no duplicates.
        panic!( "Duplicate key" );
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

  fn remove0( &mut self, mut x: usize, r: &dyn Record  ) -> ( usize, bool ) // out bool heightDecreased
  {
    if x == 0 // key not found.
    {
      // println!( "remove0: key not found" );
      return ( x, false );
    }
    let mut height_decreased: bool = true;
    let compare = self.compare( x, r );
    if compare == 0
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
      // println!("free node {}", deleted );
      self.free_node( deleted );
    } else if compare < 0 {
      let rem = self.remove0( self.get_left( x ), r );
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
      let rem = self.remove0( self.get_right(x), r );
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

  fn dump0( &self, x: usize, r: &mut dyn Record, ixf: &IndexFile )
  {
    if x != 0
    {
      self.dump0( self.get_left( x ), r, ixf );
      if !self.parent
      {
        print!( "node={} balance={} left={} right={} ", x, self.get_balance(x), self.get_left(x), self.get_right(x) );
        self.get_record( x, r );
        r.dump();
      }
      if self.parent
      {
        let cp = self.get_child( x );
        ixf.dump0( cp, r );
      }
      self.dump0( self.get_right( x ), r, ixf );
    }
  }

  fn dump ( &self, r: &mut dyn Record, ixf: &IndexFile )
  {
    println!("IndexPage dump parent={} count={} root={} max_node={} size={}", 
      self.parent, self.count, self.root, self.max_node, self.size() );
    if self.parent
    {
      ixf.dump0( self.first_page, r );
    }
    self.dump0( self.root, r, ixf );
    println!("end IndexPage dump" );
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

pub fn set( data: &mut[u8], off: usize, mut val:u64, n: usize )
{
  for i in 0..n
  {
    data[ off + i ] = ( val & 255 ) as u8;
    val >>= 8;
  }
}
