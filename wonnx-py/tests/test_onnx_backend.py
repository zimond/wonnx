# SPDX-License-Identifier: Apache-2.0

from __future__ import absolute_import
from __future__ import division
from __future__ import print_function
from __future__ import unicode_literals

import itertools
import os
import platform
import unittest
import onnx.backend.base
import onnx.backend.test

from onnx.backend.base import BackendRep, Device, DeviceType, namedtupledict
from onnx.backend.test.runner import BackendIsNotSupposedToImplementIt
import onnx.shape_inference
import onnx.version_converter
from typing import NamedTuple, Optional, Text, Any, Tuple, Sequence
from onnx import NodeProto, ModelProto, TensorProto
import numpy  # type: ignore

# The following just executes the fake backend through the backend test
# infrastructure. Since we don't have full reference implementation of all ops
# in ONNX repo, it's impossible to produce the proper results. However, we can
# run 'checker' (that's what base Backend class does) to verify that all tests
# fed are actually well-formed ONNX models.
#
# If everything is fine, all the tests would be marked as "skipped".
#
# We don't enable report in this test because the report collection logic itself
# fails when models are mal-formed.


# This is a pytest magic variable to load extra plugins
pytest_plugins = ("onnx.backend.test.report",)

import wonnx
import numpy as np


class DummyRep(BackendRep):
    def __init__(self, inputs, outputs, outputs_shape, model):
        self.inputs = inputs
        self.outputs = outputs
        self.outputs_shape = outputs_shape
        self.session = wonnx.PySession.from_bytes(onnx._serialize(model))
        self.rtol = 1
        pass

    def run(self, inputs, rtol=1.0, **kwargs):

        dicts = {}
        for k, v in zip(self.inputs, inputs):
            if isinstance(v, np.ndarray):
                dicts[k] = v.flatten()
            else:
                tmp_v = np.array(v)
                np.reshape(tmp_v, self.outputs_shape[k])
                dicts[k] = tmp_v

        results = self.session.run(dicts)

        outputs = []
        for item in results.items():
            tmp_v = np.array(item[1])
            tmp_v = np.reshape(tmp_v, self.outputs_shape[item[0]])
            tmp_v = tmp_v.astype("float32")
            outputs.append(tmp_v)
        return outputs


class DummyBackend(onnx.backend.base.Backend):
    @classmethod
    def prepare(
        cls,
        model,  # type: ModelProto
        inputs,
        device="CPU",  # type: Text
        **kwargs,  # type: Any
    ):  # type: (...) -> Optional[onnx.backend.base.BackendRep]
        super(DummyBackend, cls).prepare(model, device, **kwargs)

        # test shape inference
        model = onnx.shape_inference.infer_shapes(model)
        inputs = [input.name for input in model.graph.input]
        outputs = [output.name for output in model.graph.output]

        outputs_shape = {}
        for output in model.graph.output:
            outputs_shape[output.name] = [
                shape.dim_value for shape in output.type.tensor_type.shape.dim
            ]

        if do_enforce_test_coverage_safelist(model):
            for node in model.graph.node:
                for i, output in enumerate(node.output):
                    if node.op_type == "Dropout" and i != 0:
                        continue
                    assert output in value_infos
                    tt = value_infos[output].type.tensor_type
                    assert tt.elem_type != TensorProto.UNDEFINED
                    for dim in tt.shape.dim:
                        assert dim.WhichOneof("value") == "dim_value"

        return DummyRep(
            inputs=inputs,
            outputs=outputs,
            model=model,
            outputs_shape=outputs_shape,
        )

    @classmethod
    def supports_device(cls, device):  # type: (Text) -> bool
        d = Device(device)
        if d.type == DeviceType.CPU:
            return True
        return False


test_coverage_safelist = set(
    [
        "bvlc_alexnet",
        "densenet121",
        "inception_v1",
        "inception_v2",
        "resnet50",
        "shufflenet",
        "SingleRelu",
        "squeezenet_old",
        "vgg19",
        "zfnet",
    ]
)


def do_enforce_test_coverage_safelist(model):  # type: (ModelProto) -> bool
    if model.graph.name not in test_coverage_safelist:
        return False
    for node in model.graph.node:
        if node.op_type in set(["RNN", "LSTM", "GRU"]):
            return False
    return True


backend_test = onnx.backend.test.BackendTest(DummyBackend, __name__)

backend_test.include(f"test_relu_[a-z,_]*")
backend_test.include(f"test_conv_[a-z,_]*")
backend_test.include(f"test_abs_[a-z,_]*")
backend_test.include(f"test_acos_[a-z,_]*")
backend_test.include(f"test_atan_[a-z,_]*")
backend_test.include(f"test_ceil_[a-z,_]*")
backend_test.include(f"test_cos_[a-z,_]*")
backend_test.include(f"test_exp_[a-z,_]*")
backend_test.include(f"test_floor_[a-z,_]*")
backend_test.include(f"test_leakyrelu_[a-z,_]*")

# Disable tests for ReduceSum because ReduceSum accepts the 'axes' list as input instead of as an attribute, and the test
# case sets the 'axes' input dynamically, which we don't support (yet?).
# backend_test.include(f"test_reduce_sum_[a-z,_]*")
backend_test.include(f"test_reduce_mean_[a-z,_]*")
backend_test.include(f"test_reduce_l1_[a-z,_]*")
backend_test.include(f"test_reduce_l2_[a-z,_]*")
backend_test.include(f"test_reduce_min_[a-z,_]*")
backend_test.include(f"test_reduce_prod_[a-z,_]*")
backend_test.include(f"test_reduce_sum_square_[a-z,_]*")
backend_test.include(f"test_reduce_max_[a-z,_]*")
backend_test.include(f"test_reduce_log_sum_[a-z,_]*")
backend_test.include(f"test_reduce_log_sum_exp_[a-z,_]*")


globals().update(backend_test.enable_report().test_cases)

if __name__ == "__main__":
    unittest.main()
