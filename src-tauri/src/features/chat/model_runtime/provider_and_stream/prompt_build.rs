fn build_tool_loop_prompt(
    prepared: &PreparedPrompt,
) -> Result<(RigMessage, Vec<RigMessage>), String> {
    let mut prompt_blocks: Vec<UserContent> = Vec::new();
    for text_block in prepared_prompt_latest_user_text_blocks(prepared) {
        prompt_blocks.push(UserContent::text(text_block));
    }
    let current_prompt_content = OneOrMany::many(prompt_blocks)
        .map_err(|_| "Request payload is empty. Provide text, image, or audio.".to_string())?;
    let current_prompt: RigMessage = RigMessage::User {
        content: current_prompt_content,
    };
    let chat_history = prepared_history_to_rig_messages(prepared)?;
    Ok((current_prompt, chat_history))
}
