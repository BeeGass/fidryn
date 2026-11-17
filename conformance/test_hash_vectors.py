"""The known answers of the hash rules verify in the second implementation."""

from __future__ import annotations

import contextlib
import copy
import io
import json
import tempfile
import unittest
from pathlib import Path
from typing import Any

import blake3
import hash_vectors


def objects_in(value: Any) -> list[dict[str, Any]]:
    """Every object inside `value`, `value` included, depth first."""
    if isinstance(value, dict):
        return [value, *(found for child in value.values() for found in objects_in(child))]
    if isinstance(value, list):
        return [found for child in value for found in objects_in(child)]
    return []


def first_object_with(value: Any, key: str) -> dict[str, Any]:
    """The first object, depth first, inside `value` that holds `key`."""
    return next(found for found in objects_in(value) if key in found)


class HashVectors(unittest.TestCase):
    def test_the_known_answers_verify(self) -> None:
        vectors = hash_vectors.load(hash_vectors.VECTORS)
        self.assertEqual(hash_vectors.mismatches(vectors), [])
        self.assertEqual(
            {section: len(vectors[section]) for section in hash_vectors.SECTIONS},
            {"provisions": 4, "notes": 2, "units": 3, "containers": 2},
        )

    def test_the_file_is_in_the_layout_that_write_gives_it(self) -> None:
        text = hash_vectors.VECTORS.read_text(encoding="utf-8")
        self.assertTrue(text.splitlines()[0].startswith('{"invented": "Every model'))
        self.assertEqual(hash_vectors.dump(hash_vectors.load(hash_vectors.VECTORS)), text)

    def test_a_changed_word_and_a_changed_digest_are_reported(self) -> None:
        vectors = hash_vectors.load(hash_vectors.VECTORS)
        changed = copy.deepcopy(vectors)
        content = changed["provisions"][0]["provision"]["items"][0]["part"]["blocks"][0]
        content["para"]["runs"][0]["words"] += " Invented."
        changed["units"][0]["layers"]["editorial"] = "0" * 64
        problems = hash_vectors.mismatches(changed)
        self.assertEqual(len(problems), 2, problems)
        self.assertTrue(problems[0].startswith("provisions[0] (a provision with text only): hash"))
        self.assertTrue(problems[1].startswith("units[0] "), problems[1])

    def test_a_number_out_of_range_is_refused_with_its_model_and_field_named(self) -> None:
        vectors = hash_vectors.load(hash_vectors.VECTORS)
        # The unit has a table cell and footnote markers, which its layers read.
        name = vectors["units"][2]["name"]
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "vectors.json"
            for key in ("colspan", "rowspan", "footnote"):
                for number in (-1, 2**32):
                    changed = copy.deepcopy(vectors)
                    first_object_with(changed["units"][2], key)[key] = number
                    text = hash_vectors.dump(changed)
                    target.write_text(text, encoding="utf-8")
                    for argv in ([str(target)], [str(target), "--write"]):
                        errors = io.StringIO()
                        with contextlib.redirect_stderr(errors):
                            status = hash_vectors.main(argv)
                        message = errors.getvalue()
                        self.assertEqual(status, 2, (key, number, argv, message))
                        self.assertIn(f"units[2] ({name}): ", message)
                        self.assertIn(f"the field {key!r} is {number}", message)
                        self.assertEqual(target.read_text(encoding="utf-8"), text)

    def test_the_order_of_notes_changes_no_layer(self) -> None:
        vectors = hash_vectors.load(hash_vectors.VECTORS)
        unit = vectors["units"][0]["unit"]
        container = vectors["containers"][0]["container"]
        for model, layers in [
            (unit, hash_vectors.unit_layers),
            (container, hash_vectors.container_layers),
        ]:
            marked = [note for note in model["notes"] if '"mark"' in json.dumps(note)]
            self.assertGreaterEqual(len(marked), 2, model["address"])
            reversed_model = copy.deepcopy(model)
            reversed_model["notes"].reverse()
            self.assertNotEqual(reversed_model["notes"], model["notes"])
            self.assertEqual(layers(reversed_model), layers(model), model["address"])

    def test_a_frame_is_a_tag_a_length_and_a_value(self) -> None:
        self.assertEqual(hash_vectors.frame(0x10, b"ab"), b"\x10\x02\x00\x00\x00\x00\x00\x00\x00ab")
        self.assertEqual(hash_vectors.frame(0x1F, b""), b"\x1f" + bytes(8))

    def test_offsets_in_a_text_are_bytes_of_its_plain_text(self) -> None:
        text = {
            "runs": [
                {"words": "“a” "},
                {"ref": {"words": "b", "target": "/zz/demo/b"}},
                {"mark": {"shown": "1", "footnote": 1}},
                {"sup": "2"},
            ]
        }
        plain = "“a” b2".encode()
        expected = (
            hash_vectors.frame(0x10, plain)
            + hash_vectors.frame(
                0x11, (8).to_bytes(8, "little") + (9).to_bytes(8, "little") + b"/zz/demo/b"
            )
            + hash_vectors.frame(0x12, (9).to_bytes(8, "little") + (10).to_bytes(8, "little"))
        )
        self.assertEqual(hash_vectors.encode_text(text), expected)

    def test_an_empty_layer_is_the_hash_of_its_domain(self) -> None:
        self.assertEqual(
            hash_vectors.enacted_layer([]),
            blake3.blake3(hash_vectors.frame(0x00, b"fidryn.layer.enacted/v1")).digest(),
        )
        self.assertEqual(
            hash_vectors.editorial_layer(None, [], [], []),
            blake3.blake3(
                hash_vectors.frame(0x00, b"fidryn.layer.editorial/v1")
                + hash_vectors.frame(0x1F, b"")
            ).digest(),
        )


if __name__ == "__main__":
    unittest.main()
