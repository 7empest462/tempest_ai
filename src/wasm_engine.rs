// Copyright (c) 2026 Robert Simens. All Rights Reserved.
// Licensed under the Tempest AI Source-Available License.
// See the LICENSE file in the project root for full license text.

use miette::{Result, miette};
use wasmtime::*;

pub struct WasmSandboxEngine {
    engine: Engine,
}

impl Default for WasmSandboxEngine {
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| panic!("Failed to initialize WasmSandboxEngine: {:?}", e))
    }
}

impl WasmSandboxEngine {
    pub fn new() -> Result<Self> {
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config).map_err(|e| miette!("Failed to create Wasmtime Engine: {:?}", e))?;
        Ok(Self { engine })
    }

    pub fn run_calculator(&self, lh: i32, rh: i32, op: &str) -> Result<i32> {
        let wat = r#"
            (module
              (func $add (param $lh i32) (param $rh i32) (result i32)
                local.get $lh
                local.get $rh
                i32.add)
              (func $sub (param $lh i32) (param $rh i32) (result i32)
                local.get $lh
                local.get $rh
                i32.sub)
              (func $mul (param $lh i32) (param $rh i32) (result i32)
                local.get $lh
                local.get $rh
                i32.mul)
              (func $div (param $lh i32) (param $rh i32) (result i32)
                local.get $lh
                local.get $rh
                i32.div_s)
              (export "add" (func $add))
              (export "sub" (func $sub))
              (export "mul" (func $mul))
              (export "div" (func $div))
            )
        "#;

        let module = Module::new(&self.engine, wat).map_err(|e| miette!("Failed to compile WAT module: {:?}", e))?;
        
        let limits = StoreLimitsBuilder::new().memory_size(1024 * 1024 * 16).build(); // 16MB cap
        let mut store = Store::new(&self.engine, limits);
        store.limiter(|s| s);
        
        // Give the execution a fuel budget of 10,000 instructions
        store.set_fuel(10_000).map_err(|e| miette!("Failed to set fuel budget: {:?}", e))?;

        let linker = Linker::new(&self.engine);
        let instance = linker.instantiate(&mut store, &module).map_err(|e| miette!("Failed to instantiate module: {:?}", e))?;
        
        let func = instance.get_typed_func::<(i32, i32), i32>(&mut store, op)
            .map_err(|_| miette!("Calculator operation '{}' not found in WASM module", op))?;
            
        let res = func.call(&mut store, (lh, rh))
            .map_err(|e| miette!("WASM execution failed: {:?}", e))?;
            
        Ok(res)
    }

    /// Verifies infinite loop detection and termination under fuel limits.
    pub fn test_infinite_loop(&self) -> Result<()> {
        let wat = r#"
            (module
              (func $loop
                (loop $l
                  br $l)
              )
              (export "infinite_loop" (func $loop))
            )
        "#;
        
        let module = Module::new(&self.engine, wat).map_err(|e| miette!("Failed to compile loop WAT: {:?}", e))?;
        
        let limits = StoreLimitsBuilder::new().memory_size(1024 * 1024 * 16).build();
        let mut store = Store::new(&self.engine, limits);
        store.limiter(|s| s);
        
        // Set low fuel budget to trigger immediate termination
        store.set_fuel(500).map_err(|e| miette!("Failed to set low fuel budget: {:?}", e))?;
        
        let linker = Linker::new(&self.engine);
        let instance = linker.instantiate(&mut store, &module).map_err(|e| miette!("Failed to instantiate loop module: {:?}", e))?;
        
        let func = instance.get_typed_func::<(), ()>(&mut store, "infinite_loop")
            .map_err(|_| miette!("infinite_loop function not found"))?;
            
        let res = func.call(&mut store, ());
        
        // We EXPECT this to return an error (Trap / Out of Fuel)
        if res.is_err() {
            Ok(())
        } else {
            Err(miette!("Expected infinite loop to be interrupted by fuel limit, but it finished successfully!"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_calculator_ops() {
        let engine = WasmSandboxEngine::new().unwrap();
        
        let add = engine.run_calculator(10, 20, "add").unwrap();
        assert_eq!(add, 30);
        
        let sub = engine.run_calculator(50, 15, "sub").unwrap();
        assert_eq!(sub, 35);
        
        let mul = engine.run_calculator(6, 7, "mul").unwrap();
        assert_eq!(mul, 42);
        
        let div = engine.run_calculator(100, 4, "div").unwrap();
        assert_eq!(div, 25);
    }

    #[test]
    fn test_wasm_infinite_loop_preemption() {
        let engine = WasmSandboxEngine::new().unwrap();
        let loop_res = engine.test_infinite_loop();
        assert!(loop_res.is_ok(), "Infinite loop was not successfully interrupted by the fuel cap!");
    }
}
