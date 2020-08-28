use std::str;
use std::rc::Rc;
use std::collections::HashMap;

#[derive(Debug)]
enum Inst
{
  JumpIfFalse(usize),
  Jump(usize),
  Return,
  Throw(),
  Execute(),
  PushInt(i64),
  PushConst(Value),
  PushLocal(usize),
  PopLocal(usize),
  CompareIntLess,
  AddInt,
  Concat,
  InitFor(usize,SelectExpression),
  For(usize),
  PrintLn
}

 pub fn go( s: &str )
 {
   let mut p = Parser::new( s.as_bytes() );
   if p.parse()
   {
     let mut ee = EvalEnv::new();
     let ( ilist, local_types ) = p.finish();
     ee.alloc_locals( &local_types );
     exec( &ilist, &mut ee );
   }
}

fn exec( ilist:&[Inst], ee: &mut EvalEnv )
{
  let n = ilist.len();
  let mut ip = 0;
  while ip < n
  {
    let i = &ilist[ ip ];
    ip += 1;
    match i
    {
      Inst::Jump( x ) => ip = *x,
      Inst::JumpIfFalse( x ) => if !ee.pop_bool() { ip = *x; },
      Inst::Return => break,
      Inst::PushLocal( x ) => ee.push_local( *x ),
      Inst::PopLocal( x ) => ee.pop_local( *x ),
      Inst::PushInt( x ) => ee.push_int( *x ),
      Inst::PushConst( x ) => ee.push_const( (*x).clone() ),
      Inst::CompareIntLess => ee.compare_int_less(),
      Inst::AddInt => ee.add_int(),
      Inst::Concat => ee.concat(),
      Inst::PrintLn => ee.println(),
      _ => { panic!( "{:?}", i ); }
    }
  }
}

impl <'a> Parser <'a>
{
  fn finish( self ) -> (Vec<Inst>,Vec<DataType>)
  {
    ( self.ilist, self.local_types )
  }

  fn resolve_jumps( &mut self )
  {
    for i in 0..self.ilist.len()
    {
      match &mut self.ilist[i]
      {
        Inst::JumpIfFalse( x ) => *x = self.jumps[*x],
        Inst::Jump( x ) => *x = self.jumps[*x],
        _ => {}
      }
    }
    self.jumps.clear();
  }

  fn parse( &mut self ) -> bool
  {
    let x = self.statements();
    if let Err(e) = x
    {
      println!( "Error: {:?}", e );
      false
    } else {
      self.resolve_jumps();
      println!( "Insts");
      for (i,s) in self.ilist.iter().enumerate()
      {
        println!( "{}: {:?}", i, s );
      }

      println!( "Local variables" );
      for (k,v) in self.local_names.iter()
      {
        println!( "{} {} {:?}", v, k, self.local_types[*v] );
      }
/*
      println!( "Jumps");
      for (i,jump) in self.jumps.iter().enumerate()
      {
        println!( "{}: {}", i, jump );
      }
      println!( "Labels");
      for p in &self.labels
      {
        println!( "{}: {}", p.0, p.1 );
      }
*/
      true
    }
  }

  pub fn new( source: &'a [u8] ) -> Self
  {
    let mut result = Self
    { 
      source,
      source_ix: 0,
      cc: 0,
      token_start: 0,
      token : Token::EndOfFile,
      ns: "",  
      ts: String::new(),
      source_column : 0,
      source_line: 0,
      decimal_int: 0,
      ilist: Vec::new(),
      jumps: Vec::new(),
      labels: HashMap::new(),
      local_names: HashMap::new(),
      local_types: Vec::new(),
      break_id: 0,
      is_func: false,
    };
    result.read_char();
    result.read_token();
    result
  }

  fn read_char( &mut self ) -> u8
  {
    let cc;
    if self.source_ix >= self.source.len()
    {
      cc = 0;
      self.source_ix = self.source.len() + 1;
    } else {
      cc = self.source[ self.source_ix ];
      if cc == b'\n' 
      {
        self.source_column = 0; 
        self.source_line += 1; 
      } else {
        self.source_column += 1;
      }
      self.source_ix += 1;
    }
    self.cc = cc;
    cc
  }

  fn read_token(&mut self)
  {
    let mut cc = self.cc;
    let mut token;
    'skip_space: loop
    {
      while cc == b' ' || cc == b'\n' || cc == b'\r' { cc = self.read_char(); }
      self.token_start = self.source_ix - 1;
    
      let sc = cc; 
      cc = self.read_char();
      match sc
      {
        b'A' ..= b'Z' | b'a'..= b'z' | b'@' =>
        {
          token = Token::Name;
          while cc >= b'A' && cc <= b'Z' || cc >= b'a' && cc <= b'z' || cc == b'@' 
          { 
            cc = self.read_char(); 
          }
          self.ns = str::from_utf8(&self.source[ self.token_start..self.source_ix-1 ]).unwrap();
          self.ts = self.ns.to_uppercase();
        }
        b'0' ..= b'9' =>
        {
          token = Token::Number;
          let fc = self.source[ self.token_start ];
          if fc == b'0' && cc == b'x'
          {
            cc = self.read_char();
            token = Token::Hex;
            while cc >= b'0' && cc <= b'9' || cc >= b'A' && cc <= b'F' || cc >= b'a' && cc <= b'f'
            { cc = self.read_char(); }
          } else {
            while cc >= b'0' && cc <= b'9' { cc = self.read_char(); }
            let part1 = self.source_ix - 1;
            let s = str::from_utf8( &self.source[ self.token_start..part1 ] ).unwrap();
            self.decimal_int = s.parse().unwrap();
            if cc == b'.' && token == Token::Number
            {
              token = Token::Decimal;
              cc = self.read_char();
              while cc >= b'0' && cc <= b'9' { cc = self.read_char(); }  
              // DecimalScale = source_ix - ( part1 + 1 );
              // DecimalFrac = long.Parse( Source.Substring( part1 + 1, DecimalScale ) );
            } else {
              // DecimalScale = 0;
              // DecimalFrac = 0;
            }
          }
          self.ns = str::from_utf8(&self.source[ self.token_start..self.source_ix-1]).unwrap();
          // self.ts = self.ns.to_uppercase();
        }

        b'[' =>
        { 
          token =  Token::Name;
          let start = self.source_ix-1;
          while cc != 0
          {
            if cc == b']'
            {
              self.read_char();
              break;
            }
            cc = self.read_char();
          }
          self.ns = str::from_utf8(&self.source[ start..self.source_ix-2 ]).unwrap();
        }

        b'\'' =>
        {
          token =  Token::String;
          let mut start = self.source_ix-1; 
          self.ts = String::new();
          while cc != 0
          {
            if cc == b'\''
            {
              cc = self.read_char();
              if cc != b'\'' { break; }
              self.ts.push_str( str::from_utf8(&self.source[ start..self.source_ix-start-2 ]).unwrap() );
              start = self.source_ix;
            }
            cc = self.read_char();
          }
          self.ts.push_str( str::from_utf8(&self.source[ start..self.source_ix-2 ]).unwrap() );
          break;
        }

        b'-' =>
        {
          token =  Token::Minus; 
          if cc == b'-' // Skip single line comment.
          {
            while cc != b'\n' && cc != 0 
            { 
              cc = self.read_char();
            }
            continue 'skip_space;
          }
        }

        b'/' =>
        {
          token =  Token::Divide;
          if cc == b'*'  // Skip comment.
          {           
            cc = self.read_char();
            let mut prevchar = b'X';
            while ( cc != b'/' || prevchar != b'*' ) && cc != 0
            {
              prevchar = cc;
              cc = self.read_char();
            }
            cc = self.read_char();
            continue 'skip_space;
          }
        }
        b'>' =>
        {
          token =  Token::Greater; 
          if cc == b'=' { token =  Token::GreaterEqual; self.read_char(); }
        }
        b'<' =>
        {
          token =  Token::Less; 
          if cc == b'=' { token =  Token::LessEqual; self.read_char(); } 
          else if cc == b'>' { token =  Token::NotEqual; self.read_char(); } 
        }
        b'!' =>
        {
          token =  Token::Exclamation  ; 
          if cc == b'=' { token =  Token::NotEqual; self.read_char(); }
        }
        b'(' => token = Token::LBra,
        b')' => token = Token::RBra,
        b'|' => token = Token::VBar,
        b',' => token = Token::Comma,
        b'.' => token = Token::Dot,
        b'=' => token = Token::Equal,
        b'+' => token = Token::Plus,
        b':' => token = Token::Colon,
        b'*' => token = Token::Times,
        b'%' => token = Token::Percent,
        0    => token = Token::EndOfFile,
        _    => token = Token::Unknown
      }
      break;
    } // skip_space loop
    self.token = token;  
    // println!("Got token {:?}", token );
  } 

  // ****************** Helper functions for parsing.

  fn get_exact_data_type( &mut self, tname: &str ) -> Result< DataType, SqlError >
  {
    let result =
    match tname
    {
      "int"      => DataType::Int,
      "string"   => DataType::String,
      "binary"   => DataType::Binary,
      "tinyint"  => DataType::Tinyint,
      "smallint" => DataType::Smallint,
      "bigint"   => DataType::Bigint,
      "float"    => DataType::Float,
      "double"   => DataType::Double,
      "bool"     => DataType::Bool,
      "decimal"  =>
      {
         let mut _p = 18; let _s = 0;
         if self.test( Token::LBra )
         {
           _p = self.read_int()?;
/*
           if ( p < 1 ) Error( "Minimum precision is 1" );
           if ( p > 18 ) Error( "Maxiumum decimal precision is 18" );
           if ( test( Token::Comma) ) s = read_int();
           if ( s < 0 ) Error( "Scale cannot be negative" );
           if ( s > p ) Error( "Scale cannot be greater than precision" );
*/
           self.read( Token::RBra )?;
         }
         // return DTI.Decimal( p, s );
         DataType::Decimal
      }
      _ => { self.error( "Datatype expected".to_string() ); DataType::None }
    };
    Ok(result)
  }

  fn get_data_type( &mut self, s: &str ) -> Result< DataType, SqlError > 
  {
    // DTI::Base( self.get_exact_data_type( s ) )
    self.get_exact_data_type( s )
  }

  fn get_operator( &mut self ) -> (Token,i8)
  {
    let mut t = self.token;
    if t >= Token::Name
    {
      if t == Token::Name
      {
        t = match self.ns
        {
          "AND" => Token::And,
          "OR" => Token::Or,
          "IN" => Token::In,
          _    => return (t,-1)
        }
      }
      else { return (t,-1); }
    }
    ( t, PRECEDENCE[ t as usize ] )
  }

  fn name( &mut self ) -> Result<String, SqlError>
  {
    if self.token != Token::Name 
    { 
      Err( self.error ("Name expected".to_string() ))
    } else {
      let result = self.ns.to_string();
      self.read_token();
      Ok(result)
    }
  }

  fn local( &mut self ) -> Result<usize, SqlError>
  {
    let name = self.name()?;
    if let Some(local) = self.local_names.get( &name )
    {
      Ok(*local)
    } else {
      Err( self.error( format!( "Undeclared local: {}", name ) ) )
    }
  }

  fn read_int( &mut self ) -> Result<i64,SqlError>
  {
    if self.token != Token::Number { return Err(self.error( "Number expected".to_string() )); }
    let result = 999; // int.Parse(NS);
    self.read_token();
    Ok(result)
  }

  fn error( &self, s: String ) -> SqlError
  {
    SqlError{ line:self.source_line, column:self.source_column, msg:s  }
  }

  fn read( &mut self, t: Token ) -> Result<(),SqlError>
  {
    if self.token != t 
    {
      Err( self.error( format!("Expected {:?}", t ) ) )
    } else {
      self.read_token();
      Ok( () )
    }
  }

  fn read_name( &mut self, ts: &str ) -> Result<(),SqlError>
  {
    if self.token != Token::Name || self.ts != ts 
    {
      Err( self.error( format!("Expected {}, got{}", ts, self.ts ) ) )
    } else {
      self.read_token();
      Ok( () )
    }
  }

  fn test_name( &mut self, t: &str ) -> bool
  {
    if self.token != Token::Name || self.ts != t
    { 
      false 
    } else {
      self.read_token();
      true
    }
  }

  fn test( &mut self, t: Token ) -> bool
  {
    let result = self.token == t;
    if result { self.read_token(); }
    result   
  }

  fn add( &mut self, s: Inst )
  {
    self.ilist.push( s );
  }

  // End Help functions for parsing.


  // ****************** Expression parsing

  fn name_exp( &mut self, _agg_allowed: bool ) -> Result<Box<Expr>,SqlError>
  {
    let name = self.name()?;
    if self.test( Token::Dot )
    {
      let fname = self.name()?;
      let mut parms = Vec::new();
      self.read( Token::LBra )?;
      if self.token != Token::RBra
      {
        loop
        {
          parms.push( self.exp()? );
          if !self.test( Token::Comma ) { break; }
        }
      }
      self.read( Token::RBra )?;
      let e  = Expr::FuncCall( ExprFuncCall{ name, fname, parms } );
      Ok( Box::new(e) )
/*
    else if self.test( Token::LBra )
    {
      let mut parms = Vec::new();
      if self.token != Token::RBra 
      {
        loop
        {
          parms.push( self.exp() );
          if !self.test( Token::Comma ) { break; }
        }
      }
      self.read( Token::RBra )?;
      if agg_allowed && name == "COUNT"
      {
        if ( parms.Count > 0 { return self.error( "COUNT does have any parameters" ); }
        result = new COUNT();
      }
      else if ( agg_allowed && name == "SUM" ) result = new ExpAgg( AggOp.Sum, parms, this );
      else if ( agg_allowed && name == "MIN" ) result = new ExpAgg( AggOp.Min, parms, this );
      else if ( agg_allowed && name == "MAX" ) result = new ExpAgg( AggOp.Max, parms, this );
      else if ( name == "PARSEINT" ) result = new PARSEINT( parms, this );
      else if ( name == "PARSEDOUBLE" ) result = new PARSEDOUBLE( parms, this );
      else if ( name == "PARSEDECIMAL" ) result = new PARSEDECIMAL( parms, this );
      else if ( name == "LEN" ) result = new LEN( parms, this );
      else if ( name == "REPLACE" ) result = new REPLACE( parms, this );  
      else if ( name == "SUBSTRING" ) result = new SUBSTRING( parms, this );
      else if ( name == "EXCEPTION" ) result = new EXCEPTION( parms, this );
      else if ( name == "LASTID" ) result = new LASTID( parms, this );
      else if ( name == "GLOBAL" ) result = new GLOBAL( parms, this );   
      else if ( name == "ARG" ) result = new ARG( parms, this );   
      else if ( name == "ARGNAME" ) result = new ARGNAME( parms, this );    
      else if ( name == "FILEATTR" ) result = new FILEATTR( parms, this );
      else if ( name == "FILECONTENT" ) result = new FILECONTENT( parms, this );
      else Error( "Unknown function : " + name );
    }
    else if ( name == "true" ) result = new ExpConstant(true);
    else if ( name == "false" ) result = new ExpConstant(false);
*/
    } else {
/*
      int i = B.Lookup( name );
      if ( i < 0 )
      {
        if ( self.dyn_scope )
          result = new ExpName( name );
        else 
          Error( "Undeclared local : " + name ); 
      }
      else result = new ExpLocalVar( i, B.LocalTypeList[ i ], name );
*/
      let look = self.local_names.get( &name );
      if let Some(lnum) = look
      {
        Ok(Box::new( Expr::Local( *lnum ) ))
      } else {
        Ok(Box::new( Expr::Name( name ) ))
      }
    }
  }

  fn primary( &mut self, agg_allowed: bool ) -> Result<Box<Expr>,SqlError>
  {
    let result;
    if self.token == Token::Name
    {
      result = 
        if self.test_name( "CASE" ) 
        { 
          self.case() 
        } else if self.test_name( "NOT" ) { 
           let e = self.exp_p( 10 )?; // Not sure about precedence here.
           Ok( Box::new( Expr::Not( e ) ) )
        } else { 
          Ok( self.name_exp( agg_allowed )? )
        };
    }
    else if self.test( Token::LBra )
    {
      /* if self.test( "SELECT" )
      {
        result = self.scalar_select();
      }
      else
      */
      {
        let exp = self.exp()?;
        if self.test( Token::Comma ) // Operand of IN e.g. X IN ( 1,2,3 )
        {
          let mut list = Vec::new();
          list.push( exp );
          loop
          {
            list.push( self.exp()? );
            if !self.test( Token::Comma ) { break; }
          }
          result = Ok( Box::new(Expr::List( list ) ));
        } else {
          result = Ok( exp );
        }
      }
      self.read( Token::RBra )?;
    }
    else if self.token == Token::String
    {
      result = Ok( Box::new( Expr::Const( Value::String( Rc::new( self.ts.clone() ) ) ) ) );
      self.read_token();
    }
    else if self.token == Token::Number || self.token == Token::Decimal
    {      
      let value = self.decimal_int;
      // if ( DecimalScale > 0 ) value = value * (long)Util.PowerTen( DecimalScale ) + DecimalFrac;
      // result = new Ok( Constant( value, DecimalScale > 0 ? DTI.Decimal( 18, DecimalScale ) : DataType.Bigint );
      result = Ok( Box::new( Expr::Number( value ) ) );
      self.read_token();
/*
    } else if self.token= Token::Hex
    {
      if ( ( NS.Length & 1 ) != 0 ) Error( "Hex literal must have even number of characters" );
      result = new ExpConstant( Util.ParseHex(NS) );
      read_token();
    } else if self.test( Token::Minus ) {
      result = new ExpMinus( Exp( 30 ) );
    }
*/
    } else {
      result = Err( self.error( "Expression expected".to_string() ) );
    }
    result
  }

  fn exp_or_agg( &mut self ) -> Result<Box<Expr>,SqlError>
  {
    let pri = self.primary( true )?;
    self.exp_lp( pri, 0 )
  }

  fn exp( &mut self ) -> Result<Box<Expr>,SqlError>
  {
    // Ok( Expr::Constant( 0 ) )
    self.exp_p( 0 )
  }

  fn exp_p( &mut self, prec: i8 ) -> Result<Box<Expr>,SqlError>
  {
    let pr = self.primary( false )?;
    self.exp_lp( pr , prec )
  }
  
  fn exp_lp( &mut self, mut lhs:Box<Expr>, precedence: i8 ) -> Result<Box<Expr>,SqlError>
  {
    let (mut t, mut prec_t) = self.get_operator();
    while prec_t >= precedence
    {
      let prec_op = prec_t; 
      let op = t;
      self.read_token();
      let mut rhs = self.primary( false )?;
      let z = self.get_operator(); t = z.0; prec_t = z.1;
      while prec_t > prec_op /* or t is right-associative and prec_t == prec_op */
      {
        rhs = self.exp_lp( rhs, prec_t )?;
        let z = self.get_operator(); t = z.0; prec_t = z.1;
      }
      lhs = Box::new( Expr::Binary( ( op, lhs, rhs ) ) );
    }
    Ok(lhs)
  }

  fn case( &mut self ) -> Result<Box<Expr>,SqlError>
  {
    let mut list = Vec::new();
    while self.test_name( "WHEN" )
    {
      let test = self.exp()?; 
      self.read_name( "THEN" )?; 
      let e = self.exp()?;
      list.push( ( test, e ) );          
    }
    if list.is_empty()
    { 
      return Err( self.error( "Empty Case Expression".to_string() ) ); 
    }
    self.read_name( "ELSE" )?;
    list.push( ( Box::new(Expr::Null), self.exp()? ) );
    self.read_name( "END" )?;
    Ok( Box::new( Expr::Case( list ) ) )
  }

/*
  Exp ScalarSelect()
  {
    TableExpression te = Expressions( null );
    if ( te.ColumnCount != 1 ) Error ( "Scalar select must have one column" );
    return new ScalarSelect( te );    
  }

  // ****************** Table expression parsing

  TableExpression InsertExpression()
  {
    if ( test( "VALUES" ) ) return Values();
    else if ( test( "SELECT") ) return Expressions( null );
    else Error( "VALUES or SELECT expected" );
    return null;
  }

  TableExpression Values()
  {
    var values = new G.List<Exp[]>();
    DataType [] types = null;
    while ( true )
    {
      read( Token::LBra );
      var v = new G.List<Exp>();;
      while ( true )
      {
        v.Add( Exp() );
        if ( test( Token::RBra ) ) break;
        if ( self.token != Token::Comma ) Error( "Comma or closing bracket expected" );
        read_token();
      }
      if ( types == null )
      {
        types = new DataType[ v.Count ];
        for ( int i = 0; i < v.Count; i += 1 ) types[ i ] = v[ i ].Type;
      }
      else
      {
        if ( types.Length != v.Count ) Error( "Inconsistent number of values" );
      }
      values.Add( v.ToArray() );
      if  ( !test( Token::Comma ) && self.token != Token::LBra ) break; // The comma between multiple VALUES is optional.           
    }  
    return new ValueTable( types.Length, values );
  }
*/

  fn expressions( &mut self, set_or_for: bool ) -> Result<SelectExpression,SqlError>
  {
    // let save = self.dyn_scope; self.dyn_scope = true; // Suppresses Binding of expressions until table is known.
    let mut exps = Vec::new();
    let mut assigns = Vec::new();
    loop
    {
      if set_or_for
      {
        let local = self.local()?;
        self.read( Token::Equal )?; 
        assigns.push( local );
      }
      let exp = self.exp_or_agg()?;
      // if ( self.test( "AS" ) ) exp.Name = self.name();
      exps.push( exp );
      if !self.test( Token::Comma ) { break; }
    }

    let mut from = None;
    if self.test_name( "FROM" )
    {
      from = Some( Box::new( self.primary_table_exp()? ) );
    }

    Ok( SelectExpression{ assigns, exps, from } )

/*
    TableExpression te = test( "FROM" ) ? PrimaryTableExp() : new DummyFrom();

    if ( ObjectName == null ) te.CheckNames( this );

    Exp where = test( "WHERE" ) ? Exp() : null;

    Exp[] group = null;
    if ( test( "GROUP" ) )
    {
      var list = new G.List<Exp>();
      read( "BY" );
      do
      {
        Exp exp = Exp();
        list.Add( exp );
      } while ( test( Token::Comma ) );
      group = list.ToArray();
    }
    OrderByExp[] order = OrderBy();
    self.dyn_scope = save;

    TableExpression result;

    if ( !self.parse_only )
    {
      var save1 = Used; var save2 = CI;
      te = te.Load( this );
      CI = te.CI;
      Used = new bool[ CI.Count ]; // Bitmap of columns that are referenced by any expression.

      for ( int i=0; i<exps.Count; i+=1 ) 
      {
        if ( exps[ i ].GetAggOp() != AggOp.None )
        {
          if ( group == null ) 
          {
            group = new Exp[0];
          }
          exps[ i ].BindAgg( this );
        }
        else 
        { 
          exps[ i ].Bind( this );
        }
      }

      if ( where != null )
      {
        where.Bind( this );
        if ( where.Type != DataType.Bool ) Error( "WHERE expression must be boolean" );
      }
      
      Bind( group );

      result = new Select( exps, te, where, group, order, Used, this );
      
      if ( assigns != null ) // Convert the Rhs of each assign to be compatible with the Lhs.
      {
        var types = new DataType[ assigns.Count ];
        for ( int i = 0; i < assigns.Count; i += 1 )
          types[ i ] = B.LocalTypeList[ assigns[ i ] ];
        result.Convert( types, this );
      }

      Used = save1; CI = save2;
    }
    else result = new Select( exps, te, where, group, order, null, this ); // Potentially used by CheckNames
    return result;
*/
 }

  fn table_name( &mut self ) -> Result< TableExpression, SqlError >
  {
    if self.token == Token::Name
    {
      let n1 = self.ns;
      self.read_token();
      if self.token == Token::Dot
      {
        self.read_token();
        if self.token == Token::Name
        {
          return Ok( TableExpression::Base( n1.to_string(), self.name()? ) );
        }
      }
    }
    Err( self.error( "Table or view name expected".to_string() ) )
  }

  fn primary_table_exp( &mut self ) -> Result< TableExpression, SqlError >
  {
    if self.token == Token::Name { return self.table_name(); }
/*
    else if ( test( Token::LBra ) )
    {
      read( "SELECT" );
      TableExpression te = Expressions( null );
      read( Token::RBra );
      if ( test("AS") ) te.Alias = Name();
      return te;
    }
    Error( "Table expected" );
    return null;
*/
    Err( self.error( "Table expected".to_string() ) )
  }

/*
  OrderByExp [] OrderBy()
  {
    if ( test( "ORDER" ) )
    {
      var list = new G.List<OrderByExp>();
      read("BY");
      do
      {
        list.Add( new OrderByExp( Exp(), test("DESC") ) );
      } while ( test( Token::Comma) );
      return list.ToArray();
    }
    return null;
  }

  // ****************** Inst parsing

  TableExpression Select( bool exec )
  {
    var te = Expressions( null );
    var b = B;
    if ( exec ) Add( () => b.Select( te ) );
    return te;
  }
*/

  fn eval_exps( &mut self, x: &SelectExpression ) -> Result< (), SqlError >
  {
    for e in &x.exps
    {
      self.eval( &*e )?;
    }
    let mut i = x.assigns.len();
    while i > 0
    {
      i -= 1;
      self.add( Inst::PopLocal( x.assigns[ i ] ) );
    }
    Ok(())
  }

  fn s_set( &mut self ) -> Result< (), SqlError >
  {
    let te = self.expressions( true )?;
    self.eval_exps( &te )?;
    Ok(())
  }

/*
  void Insert()
  {
    read( "INTO" );
    string schema = Name();
    read( Token::Dot );
    string tableName = Name();
    read( Token::LBra );
    var names = new G.List<string>();
    while ( true )
    {
      string name = Name();
      if ( names.Contains( name ) ) Error( "Duplicate name in insert list" );
      names.Add( name );
      if ( test( Token::RBra ) ) break;
      if ( self.token != Token::Comma ) Error( "Comma or closing bracket expected" );
      read_token();
    }

    TableExpression src = InsertExpression();  
    if ( src.ColumnCount != names.Count ) Error( "Insert count mismatch" );

    if ( !self.parse_only )
    {
      varself.token Db.GetTable( schema, tableName, this );

      int[] colIx = new int[names.Count];
      int idCol = -1;

      var types = new DataType[ names.Count ];
      for ( int i=0; i < names.Count; i += 1 ) 
      {
        int ci = t.ColumnIx( names[ i ], this );
        if ( ci == 0 ) idCol = i;
        colIx[ i ] = ci;
        types[ i ] = t.CI.Type[ ci ];
      }
      src.Convert( types, this );
      var b = B;
      Add( () => b.Insert( t, src, colIx, idCol ) );
    }
  }

  struct Assign // Is this really needed now?
  {
    public ExpName Lhs;
    public Exp Rhs;
    public Assign( string name, Exp rhs ) { Lhs = new ExpName(name); Rhs = rhs; }
  }

  void Update()
  {
    bool save = self.dyn_scope; self.dyn_scope = true;
    var te = TableName();
    read( "SET" );
    var alist = new G.List<Assign>();
    do
    {
      var name = Name();
      read( Token::Equal );
      var exp = Exp();
      alist.Add( new Assign( name, exp ) );
    } while ( test( Token::Comma ) );
    var a = alist.ToArray();
    var where = test( "WHERE" ) ? Exp() : null;
    if ( where == null ) Error( "UPDATE must have a WHERE" );
    self.dyn_scope = save;

    if ( !self.parse_only )
    {
      Tableself.token Db.GetTable( te.Schema, te.Name, this );

      var save1 = Used; var save2 = CI;
      Used = new bool[ t.CI.Count ]; // Bitmap of columns that are referenced by any expression.
      CI = t.CI;      

      int idCol = -1;
      for ( int i=0; i < a.Length; i += 1 ) 
      {        
        a[ i ].Lhs.Bind( this );
        a[ i ].Rhs.Bind( this );

        if ( a[ i ].Lhs.Type != a[ i ].Rhs.Type )
        {
          Exp conv = a[ i ].Rhs.Convert( a[ i ].Lhs.Type );
          if ( conv == null ) Error( "Update type mismatch" );
          else a[ i ].Rhs = conv;
        }
        if ( a[ i ].Lhs.ColIx == 0 ) idCol = i;
      }
      if ( where != null )
      {
        where.Bind( this );
        if ( where.Type != DataType.Bool ) Error( "WHERE expression must be boolean" );
      }
      var whereDb = where.GetDB();
      var dvs = new Exp.DV[ a.Length ];
      var ixs = new int[ a.Length ];
      for ( int i = 0; i < a.Length; i += 1 )
      {
        ixs[ i ] = a[ i ].Lhs.ColIx;
        dvs[ i ] = a[ i ].Rhs.GetDV();
      }

      var ids = where.GetIdSet( t );
      Add( () => t.Update( ixs, dvs, whereDb, idCol, ids, B ) ); 

      Used = save1; CI = save2; 
    }
  }

  void Delete()
  {
    bool save = self.dyn_scope; self.dyn_scope = true;
    read( "FROM" );
    var te = TableName();
    Exp where = test( "WHERE" ) ? Exp() : null;
    if ( where == null ) Error( "DELETE must have a WHERE" );
    self.dyn_scope = save;

    if ( !self.parse_only )
    {
      Tableself.token Db.GetTable( te.Schema, te.Name, this );
      var save1 = Used; var save2 = CI;
      Used = new bool[ t.CI.Count ]; // Bitmap of columns that are referenced by any expression.
      CI = t.CI;

      if ( where != null )
      {
        where.Bind( this );
        if ( where.Type != DataType.Bool ) Error( "WHERE expression must be boolean" );
      }
      var whereDb = where.GetDB();
      var ids = where.GetIdSet( t );

      Add( () => t.Delete( whereDb, ids, B ) );

      Used = save1; CI = save2;
    }
  }
*/

  fn execute( &mut self ) -> Result< (), SqlError >
  {
    self.read( Token::LBra )?;
    let exp = self.exp()?;
    self.read( Token::RBra )?;
    self.eval( &*exp )?;
    self.add( Inst::Execute() );
    Ok(())
  }

/*
  void Exec()
  {
    string name = Name();
    string schema = null;
    if ( test( Token::Dot ) )
    {
      schema = name;
      name = Name();
    }
    read( Token::LBra );
    var parms = new G.List<Exp>();

    if ( !test( Token::RBra ) )
    {
      parms.Add( Exp() );
      while ( test( Token::Comma ) ) parms.Add( Exp() );
      read( Token::RBra );
    }

    if ( schema != null )
    {
      if ( !self.parse_only )
      {
        var b = Db.GetRoutine( schema, name, false, this );

        // Check parameter types.
        if ( b.Params.Count != parms.Count ) Error( "Param count error calling " + name + "." + name );
        for ( int i = 0;  i < parms.Count; i += 1 )
          if ( parms[ i ].Type != b.Params.Type[ i ] ) 
            Error( "Parameter Type Error calling procedure " + name );


        var pdv = Util.GetDVList( parms.ToArray() );
        var caller = B;
        Add( () => b.ExecuteRoutine( caller, pdv ) ); 
      }   
    }
    else if ( name == "SETMODE" )
    {
      if ( parms.Count != 1 ) Error ( "SETMODE takes one param" );
      if ( !self.parse_only )
      {
        parms[ 0 ].Bind( this );
        if ( parms[ 0 ].Type != DataType.Bigint ) Error( "SETMODE param error" );
        var dl = parms[0].GetDL();
        var b = B;
        Add( () => b.SetMode( dl ) );
      }
    }
    else Error( "Unrecognised procedure" );
  }
*/

  fn s_for( &mut self ) -> Result< (), SqlError >
  {
    let te = self.expressions( true )?;

    let for_id = self.local_types.len();
    self.local_types.push( DataType::Iterator );

    self.add( Inst::InitFor( for_id, te ) );

    let start_id = self.get_jump_id();
    self.set_jump( start_id );
    let break_id = self.get_jump_id();

    self.add( Inst::For( for_id ) );

    self.add( Inst::JumpIfFalse( break_id ) );
    let save = self.break_id;
    self.break_id = break_id;
    self.statement()?;
    self.break_id = save;
    self.add( Inst::Jump( start_id ) );
    self.set_jump( break_id );
    Ok(())
  }

/*
  // ****************** Create Insts

  void CreateTable()
  {
    string schema = Name();
    read( Token::Dot );
    string tableName = Name();
    int sourceStart = SourceIx-1;
    read( Token::LBra );
    var names = new G.List<string>();
    var types = new G.List<DataType>();
    while ( true )
    {
      var name = Name();
      if ( names.Contains( name ) ) Error ( "Duplicate column name" );
      names.Add( name );
      types.Add( GetExactDataType( Name() ) );
      if ( test( Token::RBra ) ) break;
      if ( self.token != Token::Comma ) Error( "Comma or closing bracket expected" );
      read_token();
    }
    string source = Source.Substring( sourceStart, TokenStart - sourceStart );
    var ci = ColInfo.New( names, types );
    Add( () => Db.CreateTable( schema, tableName, ci, source, false, false, this ) );
  }

  void CreateView( bool alter )
  {
    string schema = Name();
    read( Token::Dot );
    string viewName = Name();
    read( "AS" );
    int sourceStart = TokenStart;
    read( "SELECT" );
    var save = self.parse_only;
    self.parse_only = true;
    var se = Select( false );
    self.parse_only = save;
    se.CheckNames( this );
    string source = Source.Substring( sourceStart, TokenStart - sourceStart );
    Add( () => Db.CreateTable( schema, viewName, se.CI, source, true, alter, this ) );
  }

  TableExpression ViewDef()
  {
    read( "SELECT" );
    return Select( false );
  }

  void CreateRoutine( bool isFunc, bool alter )
  {
    string schema = Name();
    read( Token::Dot );
    string routineName = Name();
    int sourceStart = SourceIx-1;

    Block save1 = B; bool save2 = self.parse_only; 
    B = new Block( B.Db, isFunc ); self.parse_only = true;
    DataType retType; var parms = RoutineDef( isFunc, out retType );
    B = save1; self.parse_only = save2;

    string source = Source.Substring( sourceStart, TokenStart - sourceStart );
    Add( () => Db.CreateRoutine( schema, routineName, source, isFunc, alter, this ) );
  }

  ColInfo RoutineDef( bool func, out DataType retType )
  {
    var names = new G.List<string>();
    var types = new G.List<DataType>();

    read( Token::LBra );
    while (self.token= Token::Name )
    {
      string name = Name();
      DataType type = GetDataType( Name() );
      names.Add( name ); 
      types.Add( type );
      B.Declare( name, type );
      if (self.token= Token::RBra ) break;      
      if ( self.token != Token::Comma ) Error( "Comma or closing bracket expected" );
      read_token();
    }
    read( Token::RBra );
    if ( func ) 
    { 
      read( "RETURNS" );
      retType = GetDataType( Name() );
    } else retType = DataType.None;
    read( "AS" );
    read( "BEGIN" );
    Begin();
    B.CheckLabelsDefined( this );
    return ColInfo.New( names, types );
  }

  void CreateIndex()
  { 
    string indexname = Name();
    read( "ON" );
    string schema = Name();
    read( Token::Dot );
    string tableName = Name();
    read( Token::LBra );
    var names = new G.List<string>();
    while ( true )
    {
      names.Add( Name() );
      if ( test( Token::RBra ) ) break;
      if ( self.token != Token::Comma ) Error( "Comma or closing bracket expected" );
      read_token();
    }
    Add( () => Db.CreateIndex( schema, tableName, indexname, names.ToArray(), this ) );
  }

  void Create()
  {
    if ( test( "FUNCTION" ) ) CreateRoutine( true, false );      
    else if ( test( "PROCEDURE" ) ) CreateRoutine( false, false );
    else if ( test( "TABLE" ) ) CreateTable();
    else if ( test( "VIEW" ) ) CreateView( false );
    else if ( test ("SCHEMA" ) )
    {
      string name = Name();
      Add( () => Db.CreateSchema( name, this ) );
    }
    else if ( test ("INDEX" ) ) CreateIndex();
    else Error( "Unknown keyword" );
  }

  void Alter()
  {
    if ( test( "TABLE" ) ) AlterTable();
    else if ( test( "VIEW" ) ) CreateView( true );
    else if ( test( "FUNCTION" ) ) CreateRoutine( true, true );      
    else if ( test( "PROCEDURE" ) ) CreateRoutine( false, true );
    else Error ("ALTER : TABLE,VIEW.. expected");
  }

  void Drop() 
  {
    if ( test( "TABLE" ) )
    {
      var s = Name();
      read( Token::Dot );
      var n = Name();
      Add ( () => Db.DropTable( s, n, this ) );
    }
    else if ( test( "VIEW" ) )
    {
      var s = Name();
      read( Token::Dot );
      var n = Name();
      Add ( () => Db.DropView( s, n, this ) );
    }
    else if ( test( "INDEX" ) )
    {
      var ix = Name();
      read( "ON" );
      var s = Name();
      read( Token::Dot );
      var n = Name();
      Add( () => Db.DropIndex( s, n, ix, this ) );
    }
    else if ( test( "FUNCTION" ) )
    {
      var s = Name();
      read( Token::Dot );
      var n = Name();
      Add( () => Db.DropRoutine( s, n, true, this ) );
    }  
    else if ( test( "PROCEDURE" ) )
    {
      var s = Name();
      read( Token::Dot );
      var n = Name();
      Add( () => Db.DropRoutine( s, n, false, this ) );
    }      
    else if ( test( "SCHEMA" ) )
    {
      var s = Name();
      Add( () => Db.DropSchema( s, this ) );
    }
    else Error( "DROP : TABLE,VIEW.. expected" );
  }

  void Rename()
  {
    string objtype = TS;
    if ( test("SCHEMA") )
    {
      var name = Name();
      read( "TO" );
      var newname = Name();
      Add( () => Db.RenameSchema( name, newname, this) );
    }
    else if ( test("TABLE") | test("VIEW") | test("PROCEDURE") | test ("FUNCTION") )
    {
      var sch = Name();
      read( Token::Dot );
      var name = Name();
      read( "TO" );
      var sch1 = Name();
      read( Token::Dot );
      var name1 = Name();
      Add( () => Db.RenameObject( objtype, sch, name, sch1, name1, this ) );
    }
    else Error( "RENAME : TABLE,VIEW.. expected" );
  }


  void AlterTable()
  {
    string schema = Name();
    read( Token::Dot );
    string tableName = Name();
    
    var list = new G.List<AlterAction>();
    var action = new AlterAction();
   
    do
    {
      if ( test("ADD" ) )
      {
        action.Operation = Action.Add;
        action.Name = Name();
        action.Type = GetExactDataType( Name() );
      }
      else if ( test("DROP" ) )
      {
        action.Operation = Action.Drop;
        action.Name = Name();
      }
      else if ( test("RENAME") )
      {
        action.Operation = Action.ColumnRename;
        action.Name = Name();
        read( "TO" );
        action.NewName = Name();
      }
      else if ( test("MODIFY" ) )
      {
        action.Operation = Action.Modify;
        action.Name = Name();
        action.Type = GetExactDataType( Name() );          
      }
      else break;
      list.Add( action );
    } while ( test( Token::Comma ) );
    Add( () => Db.AlterTable( schema, tableName, list, this ) );
  }
*/

  fn throw( & mut self ) -> Result< (), SqlError >
  {
    let msg = self.exp()?;
    self.eval( &*msg )?;
    self.add( Inst::Throw() );
    Ok(())
  }    

  // Other statements.

  fn declare( & mut self ) -> Result< (), SqlError >
  {
    loop
    {
      let name = self.name()?;
      let dt = self.get_data_type( self.ns)?;
      self.read_token();

      let local_id = self.local_types.len();
      self.local_types.push( dt );
      self.local_names.insert( name, local_id ); // Should check for duplicate.

      if !self.test( Token::Comma ) { break; }
    }
    // self.add( Inst::Declare( list ) );
    Ok(())
  }

  fn get_jump_id( &mut self ) -> usize
  {
    let result = self.jumps.len();
    self.jumps.push( usize::MAX );
    result
  }

  fn set_jump( &mut self, jump_id: usize ) 
  {
    self.jumps[ jump_id ] = self.ilist.len();
  }

  fn get_goto( &mut self, s: &str ) -> usize
  {
    let v = self.labels.get( s );
    match v
    {  
      Some(jump_id) => *jump_id,
      None =>
      {
        let jump_id = self.get_jump_id();
        self.labels.insert( s.to_string(), jump_id );
        jump_id
      }
    }
  }

  fn set_label( &mut self, s: &str ) -> Result< (), SqlError >
  {
    let v = self.labels.get( s );
    match v
    {
      Some(jump_id) => 
      {
        let j = *jump_id;
        if self.jumps[ j ] != usize::MAX
        {
          Err( self.error( "Label already set".to_string() ) )
        } else {
          self.set_jump( j );
          Ok(())
        }
      }
      None =>
      {
        let jump_id = self.get_jump_id();
        self.labels.insert( s.to_string(), jump_id );
        self.set_jump( jump_id );
        Ok(())
      }
    }
  }

  fn s_while( &mut self ) -> Result< (), SqlError >
  {
    let exp = self.exp()?;
    let start_id = self.get_jump_id();
    self.set_jump( start_id );
    let break_id = self.get_jump_id();
    self.eval( &*exp )?;
    self.add( Inst::JumpIfFalse( break_id ) );
    let save = self.break_id;
    self.break_id = break_id;
    self.statement()?;
    self.break_id = save;
    self.add( Inst::Jump( start_id ) );
    self.set_jump( break_id );
    Ok(())
  }

  fn eval( &mut self, e: &Expr ) -> Result< (), SqlError >
  {
    match e 
    {
      Expr::Number( x ) => { self.add( Inst::PushInt( *x ) ); },
      Expr::Const( x ) => { self.add( Inst::PushConst( (*x).clone() ) ); },
      Expr::Binary( (op, lhs, rhs) ) =>
      {
        self.eval( lhs )?;
        self.eval( rhs )?;
        match op
        {
          Token::Less => self.add( Inst::CompareIntLess ),
          Token::Plus => self.add( Inst::AddInt ),
          Token::VBar => self.add( Inst::Concat ),
          _ => { panic!("ToDo {:?}", op ); }
        }
      }
      Expr::Local( x ) => { self.add( Inst::PushLocal( *x ) ); }
      _ => { panic!("ToDo {:?}", e ); }
    }
    Ok(())
  }

  fn s_if( &mut self ) -> Result< (), SqlError >
  {
    let exp = self.exp()?;
    let false_id = self.get_jump_id();
    self.eval( &*exp )?;
    self.add( Inst::JumpIfFalse( false_id ) );
    self.statement()?;
    if self.test_name("ELSE")
    {
      let end_id = self.get_jump_id();
      self.add( Inst::Jump( end_id ) ); // Skip over the else clause
      self.set_jump( false_id );
      self.statement()?;
      self.set_jump( end_id );
    } else {
      self.set_jump( false_id );
    }  
    Ok(())
  }

  fn s_goto( &mut self ) -> Result< (), SqlError >
  {
    let label = self.name()?;
    let to = self.get_goto( &label );
    self.add( Inst::Jump( to ) );
    Ok(())
  }

  fn s_break( &mut self ) -> Result< (), SqlError >
  {
    let break_id = self.break_id; // Need to take a copy of current value.
    if break_id == usize::MAX 
    {
      Err( self.error( "No enclosing loop for break".to_string() ) )
    } else {
      self.add( Inst::Jump( break_id ) );
      Ok(())
    }
  }

  fn s_return( &mut self ) -> Result< (), SqlError >
  {
    if self.is_func
    {
      let e = self.exp()?;
      self.eval( &*e )?
    }
    self.add( Inst::Return );
    Ok(())
  }

  fn s_begin( &mut self ) -> Result< (), SqlError >
  { 
    while !self.test_name( "END" ) 
    {
      self.statement()?;
    } 
    Ok(())
  }

  fn s_print_ln( &mut self ) -> Result< (), SqlError >
  {
    let e = self.exp()?;
    self.eval( &*e )?;
    self.add( Inst::PrintLn );
    Ok(())
  }

  fn statements( &mut self ) -> Result< (), SqlError >
  {
    loop
    {
      self.statement()?;
      if self.token == Token::EndOfFile { break; }
    }
    Ok(())
  }

  fn statement( &mut self ) -> Result<(), SqlError>
  {
    if self.token == Token::Name 
    {
      let ns = self.ns;
      self.read_token();
      if self.test( Token::Colon )
      {
        self.set_label( ns )
      } else {
        match ns
        {  
        // "ALTER" => self.alter(),
        "BEGIN" => self.s_begin(),
        "BREAK" => self.s_break(),
        // "CREATE" => self.create(),
        // "DROP" => self.drop(),
        "DECLARE" => self.declare(),
        // "DELETE" => self.delete(),
        // "EXEC" => self.exec(),
        "EXECUTE" => self.execute(),
        "FOR" => self.s_for(),
        "GOTO" => self.s_goto(),
        "IF" => self.s_if(),
        // "INSERT" => self.insert(),
        // "RENAME" => self.rename(),
        "RETURN" => self.s_return(),
        // "SELECT":  self.select( true ),
        "SET" => self.s_set(),
        "THROW" => self.throw(),
        // "UPDATE" => self.update(),
        "WHILE" => self.s_while(),
        "PRINTLN" => self.s_print_ln(),
        _ => Err( self.error( format!( "Statement keyword expected, got {}", self.ns ) ) )
        }
      }
    } else {
      Err( self.error( format!( "statement keyword expected, got {:?}", self.token ) ) )
    }
  } // end fn statement
} // end impl Parser


// *****************************************************************


pub struct Parser <'a>
{
  // Token fields.
  source: &'a [u8],
  source_ix: usize,
  cc: u8,
  token_start: usize,
  token : Token,
  ns: &'a str,  
  ts: String,
  source_column : usize,
  source_line: usize,
  decimal_int: i64,

  // Output fields.
  ilist: Vec<Inst>,
  jumps: Vec<usize>,
  labels: HashMap<String,usize>,
  local_names: HashMap<String,usize>,
  local_types: Vec<DataType>,
  break_id: usize,
  is_func: bool,
}


#[derive(Debug)]
pub struct SqlError
{
  line: usize,
  column: usize,
  msg: String
}

#[derive(Debug)]
struct ExprBinary
{
  op: Token,
  lhs: Box<Expr>,
  rhs: Box<Expr>
}

#[derive(Debug)]
struct ExprFuncCall
{
  name: String,
  fname: String,
  parms: Vec<Box<Expr>>
}

#[derive(Debug)]
enum TableExpression
{
  // Select( SelectExpression ),
  Base( String, String )
}

#[derive(Debug)]
struct SelectExpression
{
  assigns: Vec<usize>, exps: Vec<Box<Expr>>, from: Option<Box<TableExpression>>
}

#[derive(PartialEq,PartialOrd,Clone,Copy,Debug)]
enum Token { /* Note: order is significant */
  Less, LessEqual, GreaterEqual, Greater, Equal, NotEqual, In,
  Plus, Minus, Times, Divide, Percent, VBar, And, Or,
  Name, Number, Decimal, Hex, String, LBra, RBra, Comma, Colon, Dot, Exclamation, Unknown, EndOfFile
}

const PRECEDENCE : [i8;15 ] = [ 10, 10, 10, 10, 10, 10, 10, 20, 20, 30, 30, 30, 15, 8, 5 ];

#[derive(Debug)]
enum DataType { None=0, Binary=1, String=2, Bigint=3, Double=4, Int=5, Float=6, 
  Smallint=7, Tinyint=8, Bool=9, /*ScaledInt=10,*/ Iterator=11, Decimal=15 
}

#[derive(Debug)]
enum Expr 
{
  // cf https://docs.rs/syn/0.15.44/syn/enum.Expr.html
  Null,
  Local(usize),
  Number(i64),
  Const(Value),
  Binary( (Token,Box<Expr>,Box<Expr>) ),
  Not(Box<Expr>),
  FuncCall(ExprFuncCall),
  List(Vec<Box<Expr>>),
  Name(String),
  Case(Vec<(Box<Expr>,Box<Expr>)>),
}

#[derive(Debug,Clone)]
pub enum Value
{
  Bool(bool),
  Long(i64),
  Double(f64),
  String(Rc<String>),
  Binary(Rc<Vec<u8>>),
}

pub struct EvalEnv
{
  pub stack: Vec<Value>,
  bp: usize,
  str0: Value,
  binary0: Value,
}

impl EvalEnv
{
  fn new() -> EvalEnv
  {
    EvalEnv
    { stack:Vec::new(), 
      bp:0,
      str0 : Value::String( Rc::new( String::new() ) ),
      binary0 : Value::Binary( Rc::new( Vec::new() ) ),
    }
  }

  fn alloc_locals( &mut self, dt: &[DataType] )
  {
    for t in dt
    {
      match t
      {
        DataType::Bool => self.stack.push( Value::Bool( false ) ),
        DataType::Double | DataType::Float => self.stack.push( Value::Double(0.0) ),
        DataType::String => self.stack.push( self.str0.clone() ),
        DataType::Binary => self.stack.push( self.binary0.clone() ),
        _ => self.stack.push( Value::Long(0) )
      }
    }
  }

  fn push_int( &mut self, x: i64 )
  {
    self.stack.push( Value::Long( x ) );
  }

  fn push_const( &mut self, x: Value )
  {
    self.stack.push( x );
  }

  fn pop_local( &mut self, local: usize )
  {
    self.stack[ self.bp + local ] = self.stack.pop().unwrap();
  }

  fn push_local( &mut self, local: usize )
  {
    self.stack.push( self.stack[ self.bp + local ].clone() );
  }

  fn compare_int_less( &mut self )
  {
    if let Value::Long(v2) = self.stack.pop().unwrap()
    {
      if let Value::Long(v1) = self.stack.pop().unwrap()
      {
        self.stack.push( Value::Bool( v1 < v2 ) );
      }
    }
  }

  fn pop_bool( &mut self ) -> bool
  {
    if let Value::Bool(v) = self.stack.pop().unwrap()
    {
      v
    } else {
      panic!();
    }
  }

  fn add_int( &mut self )
  {   
    if let Value::Long(v2) = self.stack.pop().unwrap()
    {
      if let Value::Long(v1) = self.stack.pop().unwrap()
      {
        self.stack.push( Value::Long( v1 + v2 ) );
        return;
      }
    } 
    panic!();
  }

  fn concat( &mut self )
  {   
    if let Value::String(s2) = self.stack.pop().unwrap()
    {
      if let Value::String(s1) = self.stack.pop().unwrap()
      {
        let result = (*s1).clone() + &*s2;
        self.stack.push( Value::String( Rc::new( result ) ) );
        return;
      }
    } 
    panic!();
  }

  fn println( &mut self )
  {
    if let Value::String(v) = self.stack.pop().unwrap()
    {
      println!( "{}", v );
    }
  }
}
