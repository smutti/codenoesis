package dev.codenoesis.fixture

import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonPrimitive

data class StrictUserDto(val id: String, val nickname: String)

fun decodeStrictUser(payload: JsonObject): StrictUserDto {
    val id = payload.getValue("id").jsonPrimitive.content
    val nickname = payload.getValue("nickname").jsonPrimitive.content
    return StrictUserDto(id, nickname)
}

suspend fun getStrictUser(id: String): StrictUserDto =
    decodeStrictUser(httpGet("/users/$id"))
