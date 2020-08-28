#[derive(Clone)]
enum Value
{
  None,
  L(i64),
  F(f64),
  S(String),
  B(Vec<u8>)
}

struct EvalEnv
{
  locals: Vec<Value>
}

type CompiledStatement = Box<dyn Fn( &mut EvalEnv )->usize>;
type CompiledExpression = Box<dyn Fn( &mut EvalEnv )->Value>;

pub struct Block
{
  statements: Vec<CompiledStatement>,
}

impl Block
{
  fn execute( &self, ee: &mut EvalEnv ) // Statement execution loop.
  {
    let last = self.statements.len();
    let mut next = 0;
    while next < last 
    {
      next = self.statements[ next ]( ee );
    }
  }

  pub fn test()
  {
    let mut b = Block{ statements:Vec::new() };

    {
      let st1 : CompiledStatement = Box::new( |_ee| { println!("hello"); 2 } );
      b.statements.push( st1 );
      let st2 : CompiledStatement = Box::new( |_ee| { println!("there"); 2 } );
      b.statements.push( st2 );
      b.statements.push( Box::new( |_ee| { println!("George"); 3 } ) );

      let mut ee = EvalEnv{ locals: vec![Value::None;5] };
      b.execute( &mut ee );
    }
  }
}