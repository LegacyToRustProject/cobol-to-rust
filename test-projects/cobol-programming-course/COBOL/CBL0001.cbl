       IDENTIFICATION DIVISION.
       PROGRAM-ID. CBL0001.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-NUM1        PIC 9(4) VALUE 1234.
       01  WS-NUM2        PIC 9(4) VALUE 5678.
       01  WS-SUM         PIC 9(5).
       01  WS-DIFF        PIC S9(5).
       01  WS-PRODUCT     PIC 9(9).
       PROCEDURE DIVISION.
           COMPUTE WS-SUM = WS-NUM1 + WS-NUM2.
           COMPUTE WS-DIFF = WS-NUM1 - WS-NUM2.
           COMPUTE WS-PRODUCT = WS-NUM1 * WS-NUM2.
           DISPLAY "NUM1:    " WS-NUM1.
           DISPLAY "NUM2:    " WS-NUM2.
           DISPLAY "SUM:     " WS-SUM.
           DISPLAY "DIFF:    " WS-DIFF.
           DISPLAY "PRODUCT: " WS-PRODUCT.
           STOP RUN.
