// Copyright 2020 Nym Technologies SA
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

use rand_distr::{Distribution, Exp};
use std::time;

pub fn sample(average_duration: time::Duration) -> time::Duration {
    // this is our internal code used by our traffic streams
    // the error is only thrown if average delay is less than 0, which will never happen
    // so call to unwrap is perfectly safe here
    let exp = Exp::new(1.0 / average_duration.as_nanos() as f64).unwrap();
    time::Duration::from_nanos(exp.sample(&mut rand::thread_rng()).round() as u64)
}
