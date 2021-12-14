{%- if activation_type is matching("Relu") -%}
    {{ activation_output }} = max({{ activation_input }}, vec4<f32>(0.0, 0.0, 0.0, 0.0));

{%- elif activation_type is matching("Sigmoid") -%}
    {{ activation_output }} = vec4<f32>(1.0, 1.0, 1.0, 1.0) / (vec4<f32>(1.0, 1.0, 1.0, 1.0) + exp(-{{ activation_input }}));

{%- elif activation_type is matching("Softsign") -%}
    let input = {{ activation_input }}; 
    {{ activation_output }} = input / (vec4<f32>(1.0, 1.0, 1.0, 1.0) + abs(input));

{%- elif activation_type is matching("Softplus") -%}
    {{ activation_output }} = log(vec4<f32>(1.0, 1.0, 1.0, 1.0) + exp({{ activation_input }}));

{%- elif activation_type is matching("Clip") -%}
    let min_clip = {{ inputs[1] }}.data[0u];
    let max_clip = {{ inputs[2] }}.data[0u];
    {{ activation_output }} = clamp(
       {{ activation_input }}, 
        vec4<f32>(min_clip, min_clip, min_clip, min_clip),
        vec4<f32>(max_clip, max_clip, max_clip, max_clip),
    );

{%- elif activation_type is matching("Celu") -%}
    let input_vec = {{ activation_input }}; 
    {{ activation_output }} = max(
            vec4<f32>(0.0, 0.0, 0.0, 0.0), 
            input_vec
        ) + min(
            vec4<f32>(0.0, 0.0, 0.0, 0.0), 
            {{ alpha }} * (exp(input_vec / {{ alpha }}) - vec4<f32>(1.0, 1.0, 1.0, 1.0))
        );

{%- elif activation_type is matching("Elu") -%}
        let input_vec = {{ activation_input }}; 
        {{ activation_output }} = max(
            vec4<f32>(0.0, 0.0, 0.0, 0.0), 
            input_vec
        ) + min(
            vec4<f32>(0.0, 0.0, 0.0, 0.0), 
            {{ alpha }} * (exp(input_vec) - vec4<f32>(1.0, 1.0, 1.0, 1.0))
        );

{%- elif activation_type is matching("Mish") -%}
    let input_vec = {{ activation_input }}; 
    {{ activation_output }} = input_vec * tanh(log(vec4<f32>(1.0, 1.0, 1.0, 1.0) + exp(input_vec)));

{%- elif activation_type is matching("LeakyRelu") -%}
    {{ activation_output }} = max({{ alpha }} * {{ activation_input }}, vec4<f32>(0.0, 0.0, 0.0, 0.0));

{%- elif activation_output != activation_input -%}
       {{ activation_output }} = {{ activation_input }};
{%- endif -%}