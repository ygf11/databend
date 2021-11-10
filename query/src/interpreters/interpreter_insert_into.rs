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

use common_datablocks::DataBlock;
use common_exception::Result;
use common_planners::InsertIntoPlan;
use common_streams::CorrectWithSchemaStream;
use common_streams::DataBlockStream;
use common_streams::SendableDataBlockStream;
use tokio_stream::StreamExt;
use common_tracing::tracing;

use crate::interpreters::Interpreter;
use crate::interpreters::InterpreterPtr;
use crate::sessions::DatabendQueryContextRef;

pub struct InsertIntoInterpreter {
    ctx: DatabendQueryContextRef,
    plan: InsertIntoPlan,
    select: Option<Arc<dyn Interpreter>>,
}

impl InsertIntoInterpreter {
    pub fn try_create(
        ctx: DatabendQueryContextRef,
        plan: InsertIntoPlan,
        select: Option<Arc<dyn Interpreter>>,
    ) -> Result<InterpreterPtr> {
        Ok(Arc::new(InsertIntoInterpreter { ctx, plan, select }))
    }
}

#[async_trait::async_trait]
impl Interpreter for InsertIntoInterpreter {
    fn name(&self) -> &str {
        "InsertIntoInterpreter"
    }

    async fn execute(&self) -> Result<SendableDataBlockStream> {

        tracing::debug!("execute insert into:{:?}", self.plan.select_plan);
        // 确定数据来源
        // 1. 如果是select来源，则异步执行
        // 2. 如果是values来源，则异步执行
        let table = self
            .ctx
            .get_table(&self.plan.db_name, &self.plan.tbl_name)?;

        let io_ctx = self.ctx.get_cluster_table_io_context()?;
        let io_ctx = Arc::new(io_ctx);

        // let select_input = self.select.execute().await?;
        match &self.select {
            Some(select_executor) => {
                let select_input_stream = select_executor.execute().await?;
                // 1. 处理类型转换
                // 1.1 input 遍历datablock
                // 1.2 将datablock转化为serial
                // 1.3 类型转换
                // 2. input stream 替换
                let stream = select_input_stream.map(move |data_block| match data_block {
                    Err(fail) => Err(fail),
                    Ok(data_block) => {
                        // let res = Self::filter_map(executor.clone(), data_block);
                        // res

                        Ok(data_block)
                    }
                });

                // self.plan.input_stream
                let inner_select_stream =
                    CorrectWithSchemaStream::new(Box::pin(stream), self.plan.schema.clone());
                {
                    let mut inner = self.plan.select_input_stream.lock();
                    (*inner).replace(Box::pin(inner_select_stream));
                }

                table.append_data(io_ctx, self.plan.clone()).await?
            }
            None => table.append_data(io_ctx, self.plan.clone()).await?,
        }

        Ok(Box::pin(DataBlockStream::create(
            self.plan.schema(),
            None,
            vec![],
        )))
    }
}
