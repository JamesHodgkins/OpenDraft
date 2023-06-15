#pragma once

/**************************************************************************************
* OpenDraft:    System Image Class                                                    *
*-------------------------------------------------------------------------------------*
* Filename:     OdSyImage.h                                                           *
* Contributors: James Hodgkins                                                        *
* Date:         June 15, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Standard OpenDraft image class                                                    *
***************************************************************************************/



#include <string>



class OdSyImage {
private:
	int width, height, alpha, id;
	std::string fileLocation;

public:
	OdSyImage(int aWidth, int aHeight, int aAlpha, const char* aFileLocation, int aId)
		: width(aWidth), height(aHeight), alpha(aAlpha), fileLocation(aFileLocation), id(aId) {}

	// Getters
	int getWidth() const { return width; }
	int getHeight() const { return height; }
	int getAlpha() const { return alpha; }
	std::string getFileLocation() const { return fileLocation; }
	int getId() const { return id; }
};