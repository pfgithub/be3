use super::{BYTES, SHADER};

#[test]
fn the_punch_uniform_fits_its_declared_binding_size() {
    let module = naga::front::wgsl::parse_str(SHADER).expect("the punch shader should parse");
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .expect("the punch shader should validate");

    let uniform = module
        .global_variables
        .iter()
        .find(|(_, variable)| variable.space == naga::AddressSpace::Uniform)
        .map(|(_, variable)| variable.ty)
        .expect("the punch shader should declare a uniform");
    let size = module.types[uniform].inner.size(module.to_ctx());

    assert_eq!(u64::from(size), BYTES);
}
