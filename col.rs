/// Heap is an array organised so the smallest element can be efficiently removed.
pub struct Heap<T>{ vec: Vec<T> }

impl<T: Ord+Copy> Heap<T> // Ord+Copy means T can be compared and copied.
{
  /* Diagram showing numbering of tree elements.
           0
       1       2
     3   4   5   6

     The fundamental invariant is that a parent element is not greater than either child.
     H[N] <= H[N*2+1] and H[N] <= H[N*2+2] 
  */

  /// Create a new heap.
  pub fn new( capacity : usize ) -> Heap<T>
  {
    Heap{ vec: Vec::with_capacity( capacity ) }
  }

  /// Get the number of elements in the heap.
  pub fn count( & self ) -> usize
  {
    self.vec.len()
  }

  // add and make allow the heap to be efficiently initialised.

  /// Add an element to the array ( not yet a heap ).
  pub fn add( &mut self, x: T ) 
  {
    self.vec.push( x );
  }

  /// Make the array into a heap.
  pub fn make( &mut self )
  {
    // Initialise the heap by making every parent not greater than both it's children.

    let count = self.vec.len();
    let mut parent = count / 2;
    while parent > 0
    {
      parent -= 1; 
      let mut check = parent;
      // Move element at check down while it is greater than a child element.
      let elem : T = self.vec[ check ];
      loop
      {
        let mut child = check * 2 + 1; 
        if child >= count { break }
        let mut ce: T = self.vec[ child ];
        if child + 1 < count
        {
          let ce2: T = self.vec[ child + 1 ];
          if ce2 < ce { child += 1; ce = ce2; }
        }
        if ce >= elem { break }
        self.vec[ check ] = ce; 
        check = child;
      }
      self.vec[ check ] = elem;  
    }
  }

  /// Insert a new element into the heap.
  pub fn insert( &mut self, elem: T )
  {
    let mut child = self.vec.len();
    self.vec.push( elem );
    // Move the new element up the tree until it is not less than it's parent.
    while child > 0
    {
      let parent = ( child - 1 ) >> 1;
      let pe: T = self.vec[ parent ];
      if elem >= pe { break }
      self.vec[ child ] = pe;
      child = parent;
    }    
    self.vec[ child ] = elem;
  }

  /// Remove and return the smallest element.
  pub fn remove ( &mut self ) -> T
  {
    // The result is element 0.
    // The last element in the heap is moved to 0, then moved down until it is not greater than a child.
    let result = self.vec[ 0 ];
    let last = self.vec.len() - 1;
    let elem = self.vec[ last ];
    self.vec.pop();
    if last > 0 
    {
      let mut parent = 0;
      loop
      {
        let mut child = parent * 2 + 1; 
        if child >= last { break }
        let mut ce = self.vec[ child ];
        if child + 1 < last
        {
          let ce2 = self.vec[ child + 1 ];
          if ce2 < ce 
          { 
            child += 1; 
            ce = ce2; 
          }
        } 
        if ce >= elem { break }
        self.vec[ parent ] = ce; 
        parent = child;  
      }
      self.vec[ parent ] = elem;
    }
    result
  }
} // end impl Heap
