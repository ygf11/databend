// Copyright 2020-2021 The Datafuse Authors.
//
// SPDX-License-Identifier: Apache-2.0.

use crate::scalars::FactoryFuncRef;
use crate::scalars::Function;
use common_datavalues::DataType;

pub struct DateTimeFunction{
    inner_function: Box<dyn Function>,
}

impl DateTimeFunction {
    pub fn register(map: FactoryFuncRef) -> Result<()> {
        let mut map = map.write();

    }   
}

impl Function for DateTimeFunction {
    fn name(&self) -> &str {
        self.inner_function.name()
    }

    fn return_type(&self, args: &[DataType]) -> Result<DataType> {
        self.inner_function.return_type(args)
    }

    fn nullable(&self, _input_schema: &DataSchema) -> Result<bool> {
        Ok(false)
    }

    fn eval(&self, columns: &[DataColumn], _input_rows: usize) -> Result<DataColumn> {
       self.inner_function.eval(columns, _input_rows)
    }

    fn num_arguments(&self) -> usize {
        self.inner_function.num_arguments()
    }

    fn variadic_arguments(&self) -> Option<(usize, usize)> {
        self.inner_function.variadic_arguments()
    }
}

