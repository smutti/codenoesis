package dev.codenoesis.fixture

import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonPrimitive

data class SafeUserDto(val id: String, val nickname: String?)

fun decodeSafeUser(payload: JsonObject): SafeUserDto {
    val id = payload.getValue("id").jsonPrimitive.content
    val nickname = payload["nickname"]?.jsonPrimitive?.contentOrNull
    return SafeUserDto(id, nickname)
}

suspend fun getSafeUser(id: String): SafeUserDto =
    decodeSafeUser(httpGet("/users/$id"))
