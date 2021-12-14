use crate::utils::{ceil, get_attribute};
use std::collections::HashMap;
use tera::{Context, Tera};

pub fn compile(
    node: &crate::onnx::NodeProto,
    dims_infos: &HashMap<String, Vec<i64>>,
    tera: &Tera,
) -> (String, u32, u32, u32) {
    // Escape unwanted characters
    let mut inputs = node.get_input().to_vec();
    let mut outputs = node.get_output().to_vec();

    let input_dims = inputs
        .iter()
        .map(|input| {
            dims_infos
                .get(input.as_str())
                .unwrap_or_else(|| panic!("{} not found", input))
        })
        .collect::<Vec<_>>();
    let output_dims = outputs
        .iter()
        .map(|output| {
            dims_infos
                .get(output.as_str())
                .unwrap_or_else(|| panic!("{} not found", output))
        })
        .collect::<Vec<_>>();
    let input_lengths = input_dims
        .iter()
        .map(|dims| dims.iter().product())
        .collect::<Vec<i64>>();

    let output_lengths = output_dims
        .iter()
        .map(|dims| dims.iter().product())
        .collect::<Vec<i64>>();

    // Escaping special characters as well as adding `var_`
    // in the beginning of the variable name to avoid collisions
    // with wgsl syntax.
    inputs = inputs
        .iter()
        .map(|input| {
            let input = input.replace(&['(', ')', ',', '\"', '.', ';', ':', '\'', '/'][..], "");
            String::from("var_") + &input
        })
        .collect::<Vec<_>>();

    outputs = outputs
        .iter()
        .map(|output| {
            let output = output.replace(&['(', ')', ',', '\"', '.', ';', ':', '\'', '/'][..], "");
            String::from("var_") + &output
        })
        .collect::<Vec<_>>();

    let mut input_chunks = vec![];
    for dims in input_dims.iter() {
        let mut chunk = vec![];
        for i in 1..dims.len() {
            chunk.push(dims[i..].iter().product::<i64>());
        }
        chunk.push(1);
        input_chunks.push(chunk);
    }

    let mut output_chunks = vec![];
    for dims in output_dims.iter() {
        let mut chunk = vec![];
        for i in 1..dims.len() {
            chunk.push(dims[i..].iter().product::<i64>());
        }
        chunk.push(1);
        output_chunks.push(chunk);
    }

    let mut context = Context::new();
    context.insert("inputs", &inputs);
    context.insert("outputs", &outputs);
    context.insert("i_lens", &input_lengths);
    context.insert("o_lens", &output_lengths);
    context.insert("i_dims", &input_dims);
    context.insert("o_dims", &output_dims);
    context.insert("i_chunks", &input_chunks);
    context.insert("o_chunks", &output_chunks);
    context.insert("op_type", &node.get_op_type());

    let (template, x, y, z) = match node.get_op_type() {
        // Map simple function
        "Abs" | "Acos" | "Asin" | "Atan" | "Ceil" | "Cos" | "Cosh" | "Exp" | "Floor" | "Log"
        | "Round" | "Sign" | "Sin" | "Sinh" | "Sqrt" | "Tan" | "Tanh" => (
            "endomorphism/map.wgsl".to_string(),
            ceil(output_lengths[0], 4) as _,
            1,
            1,
        ),
        // Copy data
        "Reshape" | "Dropout" | "Flatten" | "Squeeze" | "Softmax" => (
            "endomorphism/copy.wgsl".to_string(),
            ceil(output_lengths[0], 16) as _,
            1,
            1,
        ),
        // Arithmetic operation
        "Add" | "And" | "Div" | "Equal" | "Greater" | "GreaterOrEqual" | "Less" | "LessOrEqual"
        | "Mod" | "Mul" | "Or" | "Sub" => {
            context.insert(
                "op_type",
                match node.get_op_type() {
                    "Add" => "+",
                    "And" => "&",
                    "Div" => "/",
                    "Equal" => "==",
                    "Greater" => ">",
                    "GreaterOrEqual" => ">=",
                    "Less" => "<",
                    "LessOrEqual" => "<=",
                    "Mod" => "%",
                    "Mul" => "*",
                    "Or" => "|",
                    "Sub" => "-",
                    _ => unimplemented!(),
                },
            );
            (
                "endomorphism/arithmetic.wgsl".to_string(),
                ceil(output_lengths[0], 4) as _,
                1,
                1,
            )
        }
        // Not taking into account attributes
        "BatchNormalization" => {
            let epsilon = get_attribute("epsilon", Some(1.0), node);
            context.insert("epsilon", &epsilon);

            todo!();

            //   (
            //       "endomorphism/batchnormalization.wgsl".to_string(),
            //       (length / 4) as _,
            //       1,
            //       1,
            //   )
        }
        "Relu" | "Sigmoid" | "Softsign" | "Softplus" | "Clip" | "Celu" | "Elu" => {
            let alpha = get_attribute("alpha", Some(1.0), node);
            context.insert("alpha", &alpha);
            (
                "endomorphism/activation.wgsl".to_string(),
                ceil(output_lengths[0], 4) as _,
                1,
                1,
            )
        }
        "Concat" => (
            "matrix/concat.wgsl".to_string(),
            ceil(output_lengths[0], 256) as u32,
            1,
            1,
        ),
        op @ "MaxPool"
        | op @ "AveragePool"
        | op @ "Conv"
        | op @ "ConvRelu"
        | op @ "ConvLeakyRelu"
        | op @ "ConvMish" => {
            // TODO: Conv only support NxCxHxW for the moment.
            debug_assert!(input_dims[0].len() == 4usize);

            let auto_pad = get_attribute("auto_pad", Some("NOTSET".to_string()), node);
            let dilations = get_attribute("dilations", Some(vec![1, 1]), node);
            let kernel_shape = get_attribute::<Vec<i64>>("kernel_shape", None, node);
            let strides = get_attribute("strides", Some(vec![1, 1]), node);
            let pads = get_attribute("pads", Some(vec![0, 0, 0, 0]), node);

            let pads = match auto_pad.as_str() {
                "NOTSET" => pads.to_vec(),
                "SAME_UPPER" => {
                    let slack_0 = -strides[0] + ((kernel_shape[0] - 1) * dilations[0] + 1);
                    let slack_0_div_2 = slack_0 / 2;
                    let slack_rest_0 = slack_0 % 2;
                    let slack_1 = -strides[1] + ((kernel_shape[1] - 1) * dilations[1] + 1);
                    let slack_1_div_2 = slack_1 / 2;
                    let slack_rest_1 = slack_1 % 2;
                    vec![
                        slack_0_div_2,
                        slack_1_div_2,
                        slack_0_div_2 + slack_rest_0,
                        slack_1_div_2 + slack_rest_1,
                    ]
                }
                "SAME_LOWER" => {
                    let slack_0 = -strides[0] + ((kernel_shape[0] - 1) * dilations[0] + 1);
                    let slack_0_div_2 = slack_0 / 2;
                    let slack_rest_0 = slack_0 % 2;
                    let slack_1 = -strides[1] + ((kernel_shape[1] - 1) * dilations[1] + 1);
                    let slack_1_div_2 = slack_1 / 2;
                    let slack_rest_1 = slack_1 % 2;
                    vec![
                        slack_0_div_2 + slack_rest_0,
                        slack_1_div_2 + slack_rest_1,
                        slack_0_div_2,
                        slack_1_div_2,
                    ]
                }
                _ => unimplemented!(),
            };

            let input_dims = input_dims[0];
            let output_dims = output_dims[0];

            context.insert("original_width", &input_dims[3]);
            context.insert("width", &output_dims[3]);
            context.insert("original_height", &input_dims[2]);
            context.insert("channel", &input_dims[1]);
            context.insert("stride", &strides);
            context.insert("kernel_shape", &kernel_shape);
            context.insert("kernel_len", &(kernel_shape[0] * kernel_shape[1]));
            context.insert(
                "kernel_channel_len",
                &(kernel_shape[0] * kernel_shape[1] * input_dims[1]),
            );
            context.insert("pad", &pads);
            context.insert("dilation", &dilations);

            // GLSL shader for convolution computation
            match op {
                "MaxPool" | "AveragePool" => (
                    "pool/aggregate.wgsl".to_string(),
                    ceil(output_lengths[0], 1024) as _,
                    1,
                    1,
                ),
                "Conv" | "ConvRelu" | "ConvLeakyRelu" | "ConvMish" => {
                    // Alpha is the Leaky Relu attribute
                    let alpha = get_attribute("alpha", Some(0.01), node);
                    context.insert("alpha", &alpha);

                    // GLSL shader for convolution computation
                    if (strides == [1, 1])
                        && (kernel_shape == [1, 1])
                        && (dilations == [1, 1] && (pads == [0, 0, 0, 0]))
                        && (input_dims[1] % 16 == 0)
                        && (output_dims[1] % 4 == 0)
                    {
                        (
                            "pool/conv_kernel_1.wgsl".to_string(),
                            ceil(output_lengths[0], 1024) as _,
                            1,
                            1,
                        )
                    } else if (strides == [1, 1])
                        && (kernel_shape == [3, 3])
                        && (dilations == [1, 1])
                        && (output_dims[1] % 4 == 0)
                    {
                        (
                            "pool/conv_kernel_3.wgsl".to_string(),
                            ceil(output_lengths[0], 1024) as _,
                            1,
                            1,
                        )
                    } else {
                        (
                            "pool/conv.wgsl".to_string(),
                            ceil(output_lengths[0], 256) as _,
                            1,
                            1,
                        )
                    }
                }
                _ => panic!("Invalid Opset"),
            }
        }
        "Gemm" | "MatMul" => {
            let alpha = get_attribute("alpha", Some(1.0), node);
            let beta = get_attribute("beta", Some(1.0), node);
            context.insert("alpha", &alpha);
            context.insert("beta", &beta);

            if input_dims[0][0] == 1 {
                let threads = output_dims[0][1];
                ("matrix/gemm_1.wgsl".to_string(), threads as _, 1, 1)
            } else {
                let threads = input_dims[0][0] * input_dims[1][1] / 16;
                ("matrix/gemm.wgsl".to_string(), threads as _, 1, 1)
            }
        }
        "Sum" => {
            unimplemented!()
        }
        "Transpose" => {
            let default = (input_lengths[0]..0).collect::<Vec<_>>();
            let perms = get_attribute("perm", Some(default), node);
            let permuted_dims = perms
                .iter()
                .map(|p| output_dims[0][*p as usize])
                .collect::<Vec<_>>();

            let mut chunks = vec![];
            for i in 1..permuted_dims.len() {
                chunks.push(permuted_dims[i..].iter().product::<i64>());
            }
            chunks.push(1);

            context.insert("permuted_chunks", &chunks);

            (
                "matrix/transpose.wgsl".to_string(),
                ceil(output_lengths[0], 256) as _,
                1,
                1,
            )
        }
        op => panic!(
            "Op {} is not implemented yet! Check the README if you want to implement it 👷‍♂️👷‍♀️",
            op
        ),
    };

    let shader = tera
        .render(&template, &context)
        .expect("failed to render shader");

    (shader, x, y, z)
}
