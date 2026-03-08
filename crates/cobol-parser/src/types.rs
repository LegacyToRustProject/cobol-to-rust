use serde::{Deserialize, Serialize};

/// Represents a complete COBOL program structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CobolProgram {
    pub program_id: String,
    pub identification: IdentificationDivision,
    pub environment: Option<EnvironmentDivision>,
    pub data: Option<DataDivision>,
    pub procedure: Option<ProcedureDivision>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentificationDivision {
    pub program_id: String,
    pub author: Option<String>,
    pub date_written: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentDivision {
    pub file_controls: Vec<FileControl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileControl {
    pub name: String,
    pub assign_to: String,
    pub organization: FileOrganization,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileOrganization {
    Sequential,
    Indexed,
    Relative,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataDivision {
    pub file_section: Vec<FileDescription>,
    pub working_storage: Vec<DataItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDescription {
    pub fd_name: String,
    /// Fixed record length from `RECORD CONTAINS N CHARACTERS` clause
    pub record_len: Option<usize>,
    /// Block factor from `BLOCK CONTAINS N RECORDS`
    pub block_contains: Option<usize>,
    pub record: Vec<DataItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataItem {
    pub level: u8,
    pub name: String,
    pub picture: Option<PicClause>,
    pub value: Option<String>,
    pub redefines: Option<String>,
    pub children: Vec<DataItem>,
}

/// Represents a COBOL PIC clause parsed into structured form.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PicClause {
    pub raw: String,
    pub pic_type: PicType,
    pub integer_digits: u32,
    pub decimal_digits: u32,
    pub signed: bool,
    pub total_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PicType {
    Numeric,
    Alphanumeric,
    Alphabetic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcedureDivision {
    pub sections: Vec<Section>,
    pub paragraphs: Vec<Paragraph>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub name: String,
    pub paragraphs: Vec<Paragraph>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paragraph {
    pub name: String,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Statement {
    Display(Vec<DisplayItem>),
    Move(MoveStatement),
    Compute(ComputeStatement),
    Add(ArithmeticStatement),
    Subtract(ArithmeticStatement),
    Multiply(ArithmeticStatement),
    Divide(DivideStatement),
    Perform(PerformStatement),
    If(IfStatement),
    Read(ReadStatement),
    Write(WriteStatement),
    Open(OpenStatement),
    Close(CloseStatement),
    Accept(String),
    StopRun,
    GoBack,
    Unknown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DisplayItem {
    Variable(String),
    Literal(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveStatement {
    pub from: String,
    pub to: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeStatement {
    pub target: String,
    pub expression: String,
    pub rounded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArithmeticStatement {
    pub operand: String,
    pub to: String,
    pub giving: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DivideStatement {
    pub operand: String,
    pub into: String,
    pub giving: Option<String>,
    pub remainder: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformStatement {
    pub target: PerformTarget,
    pub varying: Option<VaryingClause>,
    pub until: Option<String>,
    pub times: Option<String>,
    pub thru: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformTarget {
    Paragraph(String),
    Inline(Vec<Statement>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaryingClause {
    pub variable: String,
    pub from: String,
    pub by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IfStatement {
    pub condition: String,
    pub then_statements: Vec<Statement>,
    pub else_statements: Vec<Statement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadStatement {
    pub file: String,
    pub into: Option<String>,
    pub at_end: Vec<Statement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteStatement {
    pub record: String,
    pub from: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenStatement {
    pub mode: OpenMode,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OpenMode {
    Input,
    Output,
    IoMode,
    Extend,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseStatement {
    pub files: Vec<String>,
}

/// Analysis report for a COBOL project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisReport {
    pub programs: Vec<ProgramSummary>,
    pub copybooks: Vec<CopybookInfo>,
    pub total_lines: usize,
    pub complexity: ComplexityLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramSummary {
    pub file_path: String,
    pub program_id: String,
    pub divisions: Vec<String>,
    pub data_items: usize,
    pub paragraphs: usize,
    pub file_io: bool,
    pub line_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopybookInfo {
    pub name: String,
    pub file_path: String,
    pub referenced_by: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComplexityLevel {
    Simple,
    Moderate,
    Complex,
}
