//! Canned `achievementsets` answers for the fake server (shape per rcheevos v12.5.0
//! `test/test_rc_client.c`): one core set with given achievements and leaderboards.

/// `achievementsets` JSON for one core set: `achievements` = (id, MemAddr),
/// `leaderboards` = (id, Mem). Shape per rcheevos v12.5.0 tests.
pub fn fake_game_set_json(
    game_id: u32,
    title: &str,
    achievements: &[(u32, &str)],
    leaderboards: &[(u32, &str)],
) -> String {
    let achievements: Vec<String> = achievements
        .iter()
        .map(|(id, mem)| {
            format!(
                "{{\"ID\":{id},\"Title\":\"Ach {id}\",\"Description\":\"D {id}\",\"Flags\":3,\
                 \"Points\":5,\"MemAddr\":\"{mem}\",\"Author\":\"x\",\"BadgeName\":\"{id:05}\",\
                 \"Created\":1367266583,\"Modified\":1376929305}}"
            )
        })
        .collect();
    let leaderboards: Vec<String> = leaderboards
        .iter()
        .map(|(id, mem)| {
            format!(
                "{{\"ID\":{id},\"Title\":\"LB {id}\",\"Description\":\"D\",\"Mem\":\"{mem}\",\
                 \"Format\":\"SCORE\"}}"
            )
        })
        .collect();
    format!(
        "{{\"Success\":true,\"GameId\":{game_id},\"Title\":\"{title}\",\"ConsoleId\":7,\
         \"ImageIconUrl\":\"http://server/Images/{game_id}.png\",\"RichPresenceGameId\":{game_id},\
         \"RichPresencePatch\":\"\",\"Sets\":[{{\"AchievementSetId\":1,\"GameId\":{game_id},\
         \"Title\":null,\"Type\":\"core\",\"ImageIconUrl\":\"http://server/Images/{game_id}.png\",\
         \"Achievements\":[{}],\"Leaderboards\":[{}]}}]}}",
        achievements.join(","),
        leaderboards.join(",")
    )
}
