#ifndef OD_DAT_DOCUMENT_CPP
#define OD_DAT_DOCUMENT_CPP

/**************************************************************************************
* OpenDraft:    Document Class                                                        *
*-------------------------------------------------------------------------------------*
* Filename:     OdDocument.cpp                                                        *
* Contributors: James Hodgkins                                                        *
* Date:         June 28, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Document class for OpenDraft framework.                                           *
***************************************************************************************/


#include "DataManager/OdDocument.h"


namespace OD::Data
{
	// Constructors and Destructors
	OdDocument::OdDocument()
	{
		name = "";
		path = "";
		version = 0;
		readOnly = false;
		modified = false;
		locked = false;
	}
	
	OdDocument::~OdDocument()
	{

	}


	// 
	// Getters and Setters
	//

	// Document Name
	std::string OdDocument::getName()
	{
		return name;
	}
	
	void OdDocument::setName(std::string aName)
	{
		name = name;
	}

	// Document Path
	std::string OdDocument::getPath()
	{
		return path;
	}
	
	void OdDocument::setPath(std::string aPath)
	{
		path = path;
	}

	// Document Version
	short OdDocument::getVersion()
	{
		return version;
	}
	void OdDocument::setVersion(short aVersion)
	{
		version = version;
	}

	// Document Read-Only
	bool OdDocument::isReadOnly()
	{
		return readOnly;
	}
	
	// Upgrade open status to read-write
	bool OdDocument::upgradeOpen()
	{
		if (locked)
		{
			return false;
		}
		else
		{
			readOnly = false;
			return true;
		}
	}

	// Document Modified
	bool OdDocument::isModified()
	{
		return modified;
	}
	
	void OdDocument::setModified(bool aModified)
	{
		modified = aModified;
	}

	// Document Locked
	bool OdDocument::isLocked()
	{
		return locked;
	}
	void OdDocument::setLocked(bool aLocked)
	{
		locked = aLocked;
	}

	// Document Save
	bool OdDocument::save()
	{
		return true;
	}

	// Document Save As
	bool OdDocument::saveAs(std::string aPath)
	{
		return true;
	}


}// namespace OD::System

#endif // OD_SY_DOCUMENT_H
