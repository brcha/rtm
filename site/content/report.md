+++
title = "Report a problem or ask for a feature"
description = "How to report a problem with rtm, or ask for something new, through its GitHub issues."
+++

Found something in rtm that does not work, or want it to do something it does not do yet? Tell us
in the project's [GitHub issues](https://github.com/brcha/rtm/issues). Every report is read.

## 1. Sign in to GitHub

Opening an issue needs a GitHub account. If you do not have one yet, signing up is free:
see GitHub's guide to
[creating an account](https://docs.github.com/en/account-and-profile/how-tos/account-management/creating-an-account-on-github).

## 2. Look for an existing report

Search the [issues](https://github.com/brcha/rtm/issues?q=is%3Aissue), open and closed. If someone
has already reported your problem or asked for the same thing, add what you know to that issue
instead of opening a new one.

## 3. Open a new issue

Go to [New issue](https://github.com/brcha/rtm/issues/new/choose) and choose **Problem report** or
**Feature request**. Give it a short title that names the program and says what goes wrong, or
what you would like, then fill in the form. GitHub's guide to
[creating an issue](https://docs.github.com/en/issues/tracking-your-work-with-issues/using-issues/creating-an-issue)
shows the steps.

## 4. Say what we need to know

The problem report form asks for:

- Which program: the desktop app (`rtmapp`), the CLI (`rtmcli`), or their MCP server for AI
  assistants.
- Which release, and how you installed it: deb, RPM, AppImage, Nix, the macOS DMG, the Windows
  installer, or the portable CLI.
- Your operating system and its version.
- Whether the todo.txt file syncs through Git or Pijul.
- What you did, what you expected, and what happened instead.
- Any error message, copied as text.

A screenshot helps for the desktop app: see GitHub's guide to
[attaching files](https://docs.github.com/en/get-started/writing-on-github/working-with-advanced-formatting/attaching-files).

The feature request form asks what you want to do and why, and how you would expect it to work.

## 5. Keep private things out

Issues are public. Replace your own todo items with made-up ones that show the same problem, and
leave out file paths, repository addresses and names you do not want published.

## What happens next

The maintainer goes through new reports, may ask questions in the issue, and plans the work that
comes out of it in the project's own tracker. The issue is closed when the change is merged, and
the change reaches you with the next release.
