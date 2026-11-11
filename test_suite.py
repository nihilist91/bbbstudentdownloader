"""
test_suite.py - Full automated test suite for BigBlueButton StudyForge.
Tests URL parsing, XML/SVG parsing, chat extraction, chapter metadata generation,
and HTML/Markdown study suite builders.
"""
import os
import unittest
from core.parser import (
    parse_bbb_url, parse_metadata, parse_shapes_svg,
    parse_chat_xml, parse_presentation_text, format_duration, format_timestamp
)
from core.study_pack import (
    generate_chat_markdown, generate_interactive_chat_html,
    generate_slide_deck_html, generate_ai_study_companion
)
from core.converter import generate_ffmetadata_file


class TestBBBParser(unittest.TestCase):
    def test_url_variations(self):
        urls = [
            ("https://bbb.example.edu/playback/presentation/2.3/d49096f2a452556e34413acf9c48068280699fba-1716216469672",
             "d49096f2a452556e34413acf9c48068280699fba-1716216469672"),
            ("https://bbb.example.edu/playback/presentation/2.0/playback.html?meetingId=meeting-xyz-123",
             "meeting-xyz-123"),
            ("https://bbb.example.edu/presentation/meeting-abc-789",
             "meeting-abc-789"),
            ("https://moodle.univ.edu/playback/presentation/2.3/session-999?token=abc",
             "session-999"),
        ]
        for u, expected_id in urls:
            mid, base_url, _ = parse_bbb_url(u)
            self.assertEqual(mid, expected_id)
            self.assertIn("/presentation/" + expected_id, base_url)

    def test_metadata_parsing(self):
        sample_xml = """<?xml version="1.0" encoding="UTF-8"?>
        <recording>
            <id>rec-12345</id>
            <start_time>1716216469672</start_time>
            <meeting id="rec-12345">
                <name>Advanced Machine Learning: Transformers</name>
            </meeting>
            <meta>
                <context>CS480 - Deep Learning</context>
                <creator>Dr. Yoshua Bengio</creator>
                <description>Attention mechanism and self-attention</description>
            </meta>
            <playback>
                <duration>5400000</duration>
            </playback>
        </recording>"""
        meta = parse_metadata(sample_xml)
        self.assertEqual(meta["name"], "Advanced Machine Learning: Transformers")
        self.assertEqual(meta["course_name"], "CS480 - Deep Learning")
        self.assertEqual(meta["presenter"], "Dr. Yoshua Bengio")
        self.assertEqual(meta["duration_formatted"], "1h 30m")
        self.assertTrue(len(meta["date_str"]) > 0)

    def test_shapes_svg_parsing(self):
        svg = """<svg xmlns="http://www.w3.org/2000/svg">
            <image id="im1" in="0" out="35.5" xlink:href="presentation/p1/slide-1.png" width="1280" height="720"/>
            <image id="im2" in="35.5" out="110.2" href="presentation/p1/slide-2.png" width="1280" height="720"/>
            <image id="im3" in="110.2" out="300" xlink:href="presentation/p1/slide-3.svg" width="1280" height="720"/>
        </svg>"""
        slides = parse_shapes_svg(svg)
        self.assertEqual(len(slides), 3)
        self.assertEqual(slides[0]["index"], 1)
        self.assertEqual(slides[0]["in"], 0.0)
        self.assertEqual(slides[0]["out"], 35.5)
        self.assertEqual(slides[1]["index"], 2)
        self.assertEqual(slides[2]["href"], "presentation/p1/slide-3.svg")

    def test_chat_parsing(self):
        chat = """<recording>
            <chat>
                <chattimeline in="12.0" target="chat" name="Alice" message="Hello! Is homework 2 due Friday?" />
                <chattimeline in="25.4" target="chat" name="Professor" message="Yes Alice, and see details at https://cs.edu/hw2" />
            </chat>
        </recording>"""
        msgs, links, questions = parse_chat_xml(chat)
        self.assertEqual(len(msgs), 2)
        self.assertEqual(len(links), 1)
        self.assertIn("https://cs.edu/hw2", links)
        self.assertEqual(len(questions), 1)
        self.assertEqual(questions[0]["sender"], "Alice")


class TestStudyPackGenerators(unittest.TestCase):
    def setUp(self):
        self.meta = {
            "name": "Lecture 01",
            "course_name": "CS101",
            "presenter": "Prof. Turing",
            "date_str": "2026-11-09",
            "duration_formatted": "1h 00m"
        }
        self.msgs = [
            {"time_sec": 10.0, "time_str": "00:10", "sender": "Alice", "message": "Good morning!", "is_question": False},
            {"time_sec": 45.0, "time_str": "00:45", "sender": "Bob", "message": "Is this on the exam?", "is_question": True},
        ]
        self.links = ["https://example.com/syllabus"]
        self.slides = [
            {"index": 1, "in": 0.0, "out": 60.0, "duration": 60.0, "href": "slide-1.png", "local_filename": "slides/slide_001.png"},
            {"index": 2, "in": 60.0, "out": 120.0, "duration": 60.0, "href": "slide-2.png", "local_filename": "slides/slide_002.png"},
        ]

    def test_markdown_chat(self):
        md = generate_chat_markdown(self.meta, self.msgs, self.links)
        self.assertIn("# 💬 Public Chat Transcript - CS101", md)
        self.assertIn("https://example.com/syllabus", md)
        self.assertIn("`00:45`", md)
        self.assertIn("❓ *Is this on the exam?*", md)

    def test_interactive_chat_html(self):
        html_doc = generate_interactive_chat_html(self.meta, self.msgs, self.links)
        self.assertIn("<!DOCTYPE html>", html_doc)
        self.assertIn("CS101", html_doc)
        self.assertIn("Alice", html_doc)
        self.assertIn("searchInput", html_doc)

    def test_slide_deck_html(self):
        deck = generate_slide_deck_html(self.meta, self.slides)
        self.assertIn("<!DOCTYPE html>", deck)
        self.assertIn("slides/slide_001.png", deck)
        self.assertIn("@media print", deck)

    def test_ai_study_companion(self):
        companion = generate_ai_study_companion(self.meta, self.slides, self.msgs, [self.msgs[1]], {"1": "Intro text"})
        self.assertIn("# 🎓 AI Study Companion: CS101", companion)
        self.assertIn("Prompt 1: High-Yield Anki Flashcards", companion)
        self.assertIn("Prompt 2: Practice Exam with Distractors", companion)
        self.assertIn("Intro text", companion)

    def test_ffmetadata_chapters(self):
        import tempfile
        out_file = os.path.join(tempfile.gettempdir(), "test_chapters.txt")
        generate_ffmetadata_file(self.meta, self.slides, out_file)
        self.assertTrue(os.path.isfile(out_file))
        with open(out_file, "r", encoding="utf-8") as f:
            content = f.read()
        self.assertIn(";FFMETADATA1", content)
        self.assertIn("[CHAPTER]", content)
        self.assertIn("START=0", content)
        self.assertIn("END=60000", content)
        self.assertIn("title=Slide 1", content)
        if os.path.isfile(out_file):
            os.remove(out_file)


if __name__ == "__main__":
    unittest.main()
