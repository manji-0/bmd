Kitchen sink
============

:Author: bmd
:Version: 1

Every reStructuredText construct bmd maps, used by render and app invariant tests.
**bold**, *italic*, ``literal``, :sub:`2`, :sup:`2`, :strike:`gone`, :math:`x^2`,
`a link <https://example.com>`_, https://example.org, and a reference_.

.. _reference: https://example.net

.. contents::

Links and footnotes
===================

A footnote [#note]_ and a citation-like reference [1]_.
See `Tables`_ for more.

.. [#note] Auto-numbered footnote body.
.. [1] Numbered footnote with a `link <https://example.org/fn>`_.

Level three
-----------

Tables
======

+--------+-----------+-------+
| Left   | Center    | Right |
+========+===========+=======+
| a      | ``code``  | 1     |
+--------+-----------+-------+
| a much longer cell that wraps | **b** | 22 |
+--------+-----------+-------+

.. list-table:: List table
   :header-rows: 1

   * - Name
     - Value
   * - `cell link <https://example.net>`_
     - 2

=====  =====
Simple table
------------
A      B
=====  =====

Lists
=====

- item one
- item two

  - nested with `link <https://nested.example>`_

    #. deeper ordered

- [ ] open task
- [x] done task

1. first
2. second

a. lettered
b. items

Term
   Definition with *emphasis*.

:Field: Field list value.

-a         Option with description.
--long     Long option.

Admonitions and blocks
======================

.. note:: Note body with ``code``.

.. tip::

   Tip body.

.. warning::

   Warning body.

.. important:: Important body.

.. caution:: Caution body.

   Quoted paragraph by indentation.

.. code-block:: rust

   fn main() {
       println!("hello {}", 42);
   }

::

   literal block

.. math::

   \int_0^1 x^2 \, dx

| Line block one
| Line block two

Media
=====

.. image:: assets/missing.png
   :alt: Block image

.. figure:: assets/figure.png

   Figure caption.

.. mermaid::

   graph TD; A-->B;

----

Final paragraph after a transition.
