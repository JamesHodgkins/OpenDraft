#ifndef OD_SY_FONT_H
#define OD_SY_FONT_H

/**************************************************************************************
* OpenDraft:    System Font Class                                                     *
*-------------------------------------------------------------------------------------*
* Filename:     OdFont.h                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         June 10, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A font wrapper class for the OpenDraft framework                                  *
***************************************************************************************/



#include <vector>


namespace OD::System
{
	// Font class
	class OdFont {
	private:
		const int id;
		const char* name;
		const char* filePath;

	public:
		// Default constructor
		OdFont(const char* aName, const char* aFilePath, int aId) : name(aName), filePath(aFilePath), id(aId) {}

		// Getters
		const char* getName() const { return name; }
		const char* getFilePath() const { return filePath; }
		int getId() const { return id; }
	};

} // namespace OD::System

#endif // OD_SY_FONT_H


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