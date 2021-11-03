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

use std::sync::Arc;

use common_base::tokio;
use common_exception::Result;
use common_planners::*;
use common_planners::{self};
use futures::TryStreamExt;
use pretty_assertions::assert_eq;

use crate::pipelines::processors::*;
use crate::pipelines::transforms::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_transform_sort() -> Result<()> {
    let ctx = crate::tests::try_create_context()?;
    let test_source = crate::tests::NumberTestData::create(ctx.clone());

    // Pipeline.
    let mut pipeline = Pipeline::create(ctx.clone());
    let a = test_source.number_source_transform_for_test(8)?;
    pipeline.add_source(Arc::new(a))?;

    let sort_expression = &[sort("number", false, false)];
    let plan = PlanBuilder::create(test_source.number_schema_for_test()?)
        .sort(sort_expression)?
        .build()?;

    //  添加sort partial 的transform
    pipeline.add_simple_transform(|| {
        Ok(Box::new(SortPartialTransform::try_create(
            plan.schema(),
            sort_expression.to_vec(),
            None,
        )?))
    })?;

    // 添加 sort merge 的tranform
    pipeline.add_simple_transform(|| {
        Ok(Box::new(SortMergeTransform::try_create(
            plan.schema(),
            sort_expression.to_vec(),
            None,
        )?))
    })?;

    println!("len:{:?}", pipeline.nums());

    if pipeline.last_pipe()?.nums() > 1 {
        println!("len:{:?}", pipeline.nums());
        pipeline.merge_processor()?;
        pipeline.add_simple_transform(|| {
            Ok(Box::new(SortMergeTransform::try_create(
                plan.schema(),
                sort_expression.to_vec(),
                None,
            )?))
        })?;
    }

    // Result.
    let stream = pipeline.execute().await?;
    let result = stream.try_collect::<Vec<_>>().await?;
    let block = &result[0];
    assert_eq!(block.num_columns(), 1);

    let expected = vec![
        "+--------+",
        "| number |",
        "+--------+",
        "| 7      |",
        "| 6      |",
        "| 5      |",
        "| 4      |",
        "| 3      |",
        "| 2      |",
        "| 1      |",
        "| 0      |",
        "+--------+",
    ];
    common_datablocks::assert_blocks_eq(expected, result.as_slice());

    Ok(())
}
