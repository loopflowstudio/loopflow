use pyo3::prelude::*;

mod engine;
mod ops;

#[pymodule]
fn loopflow(m: &Bound<'_, PyModule>) -> PyResult<()> {
    engine::register(m)?;
    ops::register(m)?;
    Ok(())
}
