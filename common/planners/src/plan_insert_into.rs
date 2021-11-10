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
use common_datavalues::DataSchemaRef;
use common_infallible::Mutex;
use common_meta_types::MetaId;
use common_streams::SendableDataBlockStream;

use crate::PlanNode;

type BlockStream =
    std::pin::Pin<Box<dyn futures::stream::Stream<Item = DataBlock> + Sync + Send + 'static>>;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct InsertIntoPlan {
    pub db_name: String,
    pub tbl_name: String,
    pub tbl_id: MetaId,
    pub schema: DataSchemaRef,
    pub select_plan: Option<Box<PlanNode>>,

    #[serde(skip, default = "InsertIntoPlan::empty_select_stream")]
    pub select_input_stream: Arc<Mutex<Option<SendableDataBlockStream>>>,

    #[serde(skip, default = "InsertIntoPlan::empty_stream")]
    pub input_stream: Arc<Mutex<Option<BlockStream>>>,
}

impl PartialEq for InsertIntoPlan {
    fn eq(&self, other: &Self) -> bool {
        self.db_name == other.db_name
            && self.tbl_name == other.tbl_name
            && self.schema == other.schema
    }
}

impl InsertIntoPlan {
    pub fn empty_stream() -> Arc<Mutex<Option<BlockStream>>> {
        Arc::new(Mutex::new(None))
    }
    pub fn empty_select_stream() -> Arc<Mutex<Option<SendableDataBlockStream>>> {
        Arc::new(Mutex::new(None))
    }
    pub fn schema(&self) -> DataSchemaRef {
        self.schema.clone()
    }
    pub fn set_input_stream(&self, input_stream: BlockStream) {
        let mut writer = self.input_stream.lock();
        *writer = Some(input_stream);
    }
}
