// Copyright 2020 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::HashMap;
use std::sync::Arc;

use common_datavalues::DataField;
use common_datavalues::DataValue;
use common_exception::ErrorCode;
use common_exception::Result;
use lazy_static::lazy_static;
use common_tracing::tracing;

use crate::aggregates::AggregateFunctionRef;
use crate::aggregates::Aggregators;

pub type AggregateFunctionCreator =
    Box<dyn Fn(&str, Vec<DataValue>, Vec<DataField>) -> Result<AggregateFunctionRef> + Sync + Send>;

pub type AggregateFunctionCombinatorCreator = Box<
    dyn Fn(
            &str,
            Vec<DataValue>,
            Vec<DataField>,
            &AggregateFunctionCreator,
        ) -> Result<AggregateFunctionRef>
        + Sync
        + Send,
>;

lazy_static! {
    static ref FACTORY: Arc<AggregateFunctionFactory> = {
        let mut factory = AggregateFunctionFactory::create();
        Aggregators::register(&mut factory);
        Aggregators::register_combinator(&mut factory);
        Arc::new(factory)
    };
}

pub struct AggregateFunctionDescription {
    aggregate_function_creator: AggregateFunctionCreator,
    // TODO(Winter): function document, this is very interesting.
    // TODO(Winter): We can support the SHOW FUNCTION DOCUMENT `function_name` or MAN FUNCTION `function_name` query syntax.
}

impl AggregateFunctionDescription {
    pub fn creator(creator: AggregateFunctionCreator) -> AggregateFunctionDescription {
        AggregateFunctionDescription {
            aggregate_function_creator: creator,
        }
    }
}

pub struct CombinatorDescription {
    creator: AggregateFunctionCombinatorCreator,
    // TODO(Winter): function document, this is very interesting.
    // TODO(Winter): We can support the SHOW FUNCTION DOCUMENT `function_name` or MAN FUNCTION `function_name` query syntax.
}

impl CombinatorDescription {
    pub fn creator(creator: AggregateFunctionCombinatorCreator) -> CombinatorDescription {
        CombinatorDescription { creator }
    }
}

pub struct AggregateFunctionFactory {
    case_insensitive_desc: HashMap<String, AggregateFunctionDescription>,
    case_insensitive_combinator_desc: Vec<(String, CombinatorDescription)>,
}

impl AggregateFunctionFactory {
    pub(in crate::aggregates::aggregate_function_factory) fn create() -> AggregateFunctionFactory {
        AggregateFunctionFactory {
            case_insensitive_desc: Default::default(),
            case_insensitive_combinator_desc: Default::default(),
        }
    }

    pub fn instance() -> &'static AggregateFunctionFactory {
        FACTORY.as_ref()
    }

    pub fn register(&mut self, name: &str, desc: AggregateFunctionDescription) {
        let case_insensitive_desc = &mut self.case_insensitive_desc;
        case_insensitive_desc.insert(name.to_lowercase(), desc);
    }

    pub fn register_combinator(&mut self, suffix: &str, desc: CombinatorDescription) {
        for (exists_suffix, _) in &self.case_insensitive_combinator_desc {
            if exists_suffix.eq_ignore_ascii_case(suffix) {
                panic!(
                    "Logical error: {} combinator suffix already exists.",
                    suffix
                );
            }
        }

        let case_insensitive_combinator_desc = &mut self.case_insensitive_combinator_desc;
        case_insensitive_combinator_desc.push((suffix.to_lowercase(), desc));
    }

    pub fn get(
        &self,
        name: impl AsRef<str>,
        params: Vec<DataValue>,
        arguments: Vec<DataField>,
    ) -> Result<AggregateFunctionRef> {
        let origin = name.as_ref();
        let lowercase_name = origin.to_lowercase();

        let aggregate_functions_map = &self.case_insensitive_desc;
        if let Some(desc) = aggregate_functions_map.get(&lowercase_name) {
            tracing::debug!("when create aggr function, nested_name: {}, params: {:?}, arguments:{:?}", origin, params, arguments);
            return (desc.aggregate_function_creator)(origin, params, arguments);
        }

        // find suffix
        for (suffix, desc) in &self.case_insensitive_combinator_desc {
            if let Some(nested_name) = lowercase_name.strip_suffix(suffix) {
                let aggregate_functions_map = &self.case_insensitive_desc;

                match aggregate_functions_map.get(nested_name) {
                    None => {
                        break;
                    }
                    Some(nested_desc) => {
                        tracing::debug!("when create aggr function, nested_name: {}, params: {:?}, arguments:{:?}", nested_name, params, arguments);
                        return (desc.creator)(
                            nested_name,
                            params,
                            arguments,
                            &nested_desc.aggregate_function_creator,
                        );
                    }
                }
            }
        }

        Err(ErrorCode::UnknownAggregateFunction(format!(
            "Unsupported AggregateFunction: {}",
            origin
        )))
    }

    pub fn check(&self, name: impl AsRef<str>) -> bool {
        let origin = name.as_ref();
        let lowercase_name = origin.to_lowercase();

        if self.case_insensitive_desc.contains_key(&lowercase_name) {
            return true;
        }

        // find suffix
        for (suffix, _) in &self.case_insensitive_combinator_desc {
            if let Some(nested_name) = lowercase_name.strip_suffix(suffix) {
                if self.case_insensitive_desc.contains_key(nested_name) {
                    return true;
                }
            }
        }

        false
    }

    pub fn registered_names(&self) -> Vec<String> {
        self.case_insensitive_desc.keys().cloned().collect()
    }
}
