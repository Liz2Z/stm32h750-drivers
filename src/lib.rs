#![no_std]

// 平台无关逻辑由宿主机单元测试与外部集成测试共同验证。
pub mod application;
#[cfg(target_arch = "arm")]
pub mod bsp;
pub mod drivers;
pub mod ui;

#[cfg(test)]
extern crate std;
