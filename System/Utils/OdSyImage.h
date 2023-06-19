#ifndef OD_SY_IMAGE_H
#define OD_SY_IMAGE_H

/**************************************************************************************
* OpenDraft:    System Image Class                                                    *
*-------------------------------------------------------------------------------------*
* Filename:     OdSyImage.h                                                           *
* Contributors: James Hodgkins                                                        *
* Date:         June 15, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   An image wrapper class for the OpenDraft framework                                 *
***************************************************************************************/



class OdSyImage {
private:
	int width, height, alpha, id;
	const char* filePath;

public:
	OdSyImage(int aWidth, int aHeight, int aAlpha, const char* aFilePath, int aId)
		: width(aWidth), height(aHeight), alpha(aAlpha), filePath(aFilePath), id(aId) {}

	// Getters
	int getWidth() const { return width; }
	int getHeight() const { return height; }
	int getAlpha() const { return alpha; }
	const char* getFilePath() const { return filePath; }
	int getId() const { return id; }
};

#endif // OD_SY_IMAGE_H