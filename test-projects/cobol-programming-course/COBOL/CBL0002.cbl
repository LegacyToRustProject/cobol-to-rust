       IDENTIFICATION DIVISION.
       PROGRAM-ID. CBL0002.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-GRADE       PIC 9(3).
       01  WS-LETTER      PIC X.
       PROCEDURE DIVISION.
           MOVE 85 TO WS-GRADE.
           EVALUATE TRUE
               WHEN WS-GRADE >= 90
                   MOVE 'A' TO WS-LETTER
               WHEN WS-GRADE >= 80
                   MOVE 'B' TO WS-LETTER
               WHEN WS-GRADE >= 70
                   MOVE 'C' TO WS-LETTER
               WHEN WS-GRADE >= 60
                   MOVE 'D' TO WS-LETTER
               WHEN OTHER
                   MOVE 'F' TO WS-LETTER
           END-EVALUATE.
           DISPLAY "GRADE: " WS-GRADE.
           DISPLAY "LETTER: " WS-LETTER.
           STOP RUN.
