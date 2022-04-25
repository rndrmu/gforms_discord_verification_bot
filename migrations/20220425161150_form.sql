-- Add migration script here
-- Add migration script here
/*
enum DiagnosisStatus {
    Formal,
    Questioning,
    SelfDiagnose, 
    FriendOrFamily
}

enum Gender {
    Male,
    Female,
    Divers
}

struct FormAnswers {
    pub discord_tag: String,
    pub status: DiagnosisStatus,
    pub gender: Gender,
    pub is_18_plus: bool,
    pub is_30_plus: bool,
    pub age: Option<String>,

}
*/

CREATE TABLE formanswers (
    message_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    age TEXT,
    gender TEXT NOT NULL,
    is_female BOOL NOT NULL DEFAULT FALSE,
    is_18_plus BOOL NOT NULL DEFAULT FALSE,
    is_30_plus BOOL NOT NULL DEFAULT FALSE,
    diagnosis_status TEXT
)