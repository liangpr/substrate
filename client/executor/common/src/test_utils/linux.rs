// This file is part of Substrate.

// Copyright (C) 2017-2021 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Implementation of Linux specific tests and/or helper functions.

use crate::wasm_runtime::WasmInstance;
use std::{
	ops::Range,
	collections::BTreeMap,
};

/// Returns how much bytes of the instance's memory is currently resident (backed by phys mem)
pub fn instance_resident_bytes(instance: &dyn WasmInstance) -> usize {
	let base_addr = instance.linear_memory_range().unwrap().start;
	Smaps::new().get_rss(base_addr).expect("failed to get rss")
}

/// An interface to the /proc/self/smaps
///
/// See docs about [procfs on kernel.org][procfs]
///
/// [procfs]: https://www.kernel.org/doc/html/latest/filesystems/proc.html
pub struct Smaps(Vec<(Range<usize>, BTreeMap<String, usize>)>);

impl Smaps {
	/// Create a in-memory representation of the calling processe's memory map.
	pub fn new() -> Self {
		let regex_start = regex::RegexBuilder::new("^([0-9a-f]+)-([0-9a-f]+)")
			.multi_line(true)
			.build()
			.unwrap();
		let regex_kv = regex::RegexBuilder::new(r#"^([^:]+):\s*(\d+) kB"#)
			.multi_line(true)
			.build()
			.unwrap();
		let smaps = std::fs::read_to_string("/proc/self/smaps").unwrap();
		let boundaries: Vec<_> = regex_start
			.find_iter(&smaps)
			.map(|matched| matched.start())
			.chain(std::iter::once(smaps.len()))
			.collect();

		let mut output = Vec::new();
		for window in boundaries.windows(2) {
			let chunk = &smaps[window[0]..window[1]];
			let caps = regex_start.captures(chunk).unwrap();
			let start = usize::from_str_radix(caps.get(1).unwrap().as_str(), 16).unwrap();
			let end = usize::from_str_radix(caps.get(2).unwrap().as_str(), 16).unwrap();

			let values = regex_kv
				.captures_iter(chunk)
				.map(|cap| {
					let key = cap.get(1).unwrap().as_str().to_owned();
					let value = cap.get(2).unwrap().as_str().parse().unwrap();
					(key, value)
				})
				.collect();

			output.push((start..end, values));
		}

		Self(output)
	}

	/// Returns how much memory is currently resident in the memory mapping that is
	/// associated with the specified address.
	pub fn get_rss(&self, addr: usize) -> Option<usize> {
		self.get_map(addr).get("Rss").cloned()
	}

	/// Get the mapping at the specified address.
	fn get_map(&self, addr: usize) -> &BTreeMap<String, usize> {
		&self.0
			.iter()
			.find(|(range, _)| addr >= range.start && addr < range.end)
			.unwrap()
			.1
	}
}