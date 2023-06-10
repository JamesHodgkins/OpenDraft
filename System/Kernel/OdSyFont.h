#pragma once

/**************************************************************************************
* OpenDraft:    System Font Class                                                     *
*-------------------------------------------------------------------------------------*
* Filename:     OdSyFont.h                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         June 10, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A font wrapper class for the OpenDraft framework                                  *
***************************************************************************************/


#include <vector>


class OdSyFont {
private:

public:
	float size;
	const char* name;

	// Default constructor
	OdSyFont(const char* n, float s = 14.f) : name(n), size(s) {}

};




//
// TO DO - GET SYSTEM FONTS
//
/*
int CALLBACK EnumFontFamExProc(
	const LOGFONTW* lpelfe,
	const TEXTMETRICW* lpntme,
	DWORD FontType,
	LPARAM lParam
) {
	std::vector<std::string>& fontNames = *reinterpret_cast<std::vector<std::string>*>(lParam);
	int bufferSize = WideCharToMultiByte(CP_UTF8, 0, lpelfe->lfFaceName, -1, nullptr, 0, nullptr, nullptr);
	std::vector<char> buffer(bufferSize);
	WideCharToMultiByte(CP_UTF8, 0, lpelfe->lfFaceName, -1, buffer.data(), bufferSize, nullptr, nullptr);
	std::string fontName(buffer.data());
	fontNames.push_back(fontName);
	return 1;
}

int listFonts() {
	// Create a vector to store the font names
	std::vector<std::string> fontNames;

	// Enumerate through all the fonts
	HDC hdc = GetDC(NULL);
	EnumFontFamiliesEx(hdc, nullptr, reinterpret_cast<FONTENUMPROC>(EnumFontFamExProc), reinterpret_cast<LPARAM>(&fontNames), 0);
	ReleaseDC(NULL, hdc);

	// Print the font names
	for (const auto& fontName : fontNames) {
		std::cout << fontName << std::endl;
	}

	return 0;
}
*/