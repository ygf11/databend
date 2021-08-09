// Copyright 2020-2021 The Datafuse Authors.
//
// SPDX-License-Identifier: Apache-2.0.

use crate::scalars::Function;
use common_datavalues::DataType;

pub struct TimeZoneFunction {

}

impl Function for TimeZoneFunction {
    fn name(&self) -> &str {
        "datetime_TimeZone"
    }

    fn return_type(&self, args: &[DataType]) -> Result<DataType> {
       Ok(DateType::Utf8)
    }

    fn nullable(&self, _input_schema: &DataSchema) -> Result<bool> {
        Ok(false)
    }

    fn eval(&self, columns: &[DataColumn], _input_rows: usize) -> Result<DataColumn> {
        
    }

    fn num_arguments(&self) -> usize {
        0
    }

    fn variadic_arguments(&self) -> Option<(usize, usize)> {
        Some((1, 2))
    }
}