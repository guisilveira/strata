# SPDX-License-Identifier: MIT
"""Recent degrades safely where the platform provides no recent-file backend.

The pinned E2E image runs with ``GIO_USE_VFS=local`` and ships no gvfs, so the
``recent`` URI scheme is genuinely unavailable here. That is the environment the
availability gate exists for, so these scenarios exercise it directly: Recent
must stay out of the sidebar, must stay out even when its preference is enabled,
and every spelling of a ``recent`` URI must be refused without disturbing the
session. Browsing a populated Recent collection needs a gvfs-backed image and is
covered by the Rust GTK tests instead.
"""

from __future__ import annotations

RECENT_URIS = ("recent:///", "recent://")


def _switch(window, name):
    return next(
        (
            node
            for node in window.find_all(name=name, rendered=False)
            if node.role in {"check box", "toggle button", "switch"}
        ),
        None,
    )


def _navigate(strata, uri):
    strata.keyboard.press("ctrl+l")
    field = strata.editable_field()
    strata.keyboard.press("ctrl+a")
    strata.keyboard.type_text(uri)
    strata.wait(lambda: field.text == uri, f"{uri} to be typed")
    strata.keyboard.press("Return")


def test_recent_is_absent_from_the_sidebar_without_a_platform_backend(strata):
    strata.sidebar_button("Home")
    strata.sidebar_button("Trash")

    assert strata.window.find(role="button", name="Recent") is None, (
        "Recent must not be offered when the recent backend is unavailable"
    )


def test_neither_recent_preference_state_can_add_an_unsupported_place(strata):
    settings = strata.window.find(role="button", name="Settings")
    assert settings is not None and settings.activate()
    strata.wait(
        lambda: _switch(strata.window, "Show Recent in sidebar"),
        "the Recent sidebar preference",
    )
    enabled = "pressed" in _switch(strata.window, "Show Recent in sidebar").states

    # Drive both states: availability, not the preference, is what withholds the
    # place, so neither value may bring it back.
    for _ in range(2):
        _switch(strata.window, "Show Recent in sidebar").activate()
        enabled = not enabled
        strata.wait(
            lambda: (
                "pressed" in _switch(strata.window, "Show Recent in sidebar").states
            )
            == enabled,
            "the Recent preference to change",
        )
        strata.wait(
            lambda: strata.window.find(role="button", name="Recent") is None,
            "Recent to stay hidden while its backend is unavailable",
        )

    strata.keyboard.press("Escape")
    # The rebuilds must leave the other default places alone.
    strata.sidebar_button("Home")
    strata.sidebar_button("Trash")


def test_every_recent_uri_spelling_is_refused_without_disturbing_the_session(strata):
    root = strata.fixture.root.name
    strata.entry("documents")

    for uri in RECENT_URIS:
        _navigate(strata, uri)

        dialog = strata.wait(
            lambda: strata.window.find(role="dialog", name="Unable to open location"),
            f"the failure report for {uri}",
        )
        assert "backend isn't installed" in dialog.dump(), (
            f"{uri} should explain why it cannot be opened"
        )
        strata.keyboard.press("Escape")
        strata.wait(
            lambda: strata.window.find(role="dialog", name="Unable to open location")
            is None,
            "the failure report to close",
        )
        # The refused location must leave the browser where it was, not on a
        # half-built column that still accepts directory actions.
        strata.wait_for_directory(root)
        strata.entry("documents")
